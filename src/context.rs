use crate::{
    Config,
    Error,
    LLMToken,
    Logger,
    LogEntry,
    ParsedSegment,
    Request,
    Thinking,
    ToolCallSuccess,
    Turn,
    TurnId,
    TurnResult,
    TurnResultSummary,
    TurnSummary,
    get_first_tool_call,
    load_available_binaries,
    request,
};
use ragit_fs::{WriteMode, join, read_string, write_string};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub struct Context {
    pub history: Vec<TurnSummary>,

    // If we have this, that means we already have LLM's response,
    // so we just have to run tool-call (or throw a parse error).
    pub curr_raw_response: Option<(String, u64)>,

    pub user_request: Option<(u64, String)>,
    pub completed_user_requests: HashSet<u64>,

    // in-memory data structures
    pub turns: HashMap<TurnId, Turn>,  // it's lazily loaded
    pub system_prompt: String,
    pub available_binaries: Vec<String>,
    pub logger: Logger,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContextJson {
    pub history: Vec<TurnId>,
    pub curr_raw_response: Option<(String, u64)>,
    pub user_request: Option<(u64, String)>,
    pub completed_user_requests: HashSet<u64>,
}

impl Context {
    pub fn new(config: &Config) -> Result<Self, Error> {
        let system_prompt = tera::Tera::one_off(
            include_str!("../system.pdl"),
            &config.system_prompt_context(),
            true,
        )?;
        let available_binaries = load_available_binaries()?;
        let logger = Logger::new();

        Ok(Context {
            history: vec![],
            curr_raw_response: None,
            user_request: None,
            completed_user_requests: HashSet::new(),
            turns: HashMap::new(),
            system_prompt,
            available_binaries,
            logger,
        })
    }

    pub fn load(config: &Config) -> Result<Self, Error> {
        let s = read_string(&join(".neukgu", "context.json")?)?;
        let context_json: ContextJson = serde_json::from_str(&s)?;

        Ok(Context {
            history: context_json.history.iter().map(
                |h| h.get_turn_summary()
            ).collect(),
            curr_raw_response: context_json.curr_raw_response.clone(),
            user_request: context_json.user_request.clone(),
            completed_user_requests: context_json.completed_user_requests.clone(),
            ..Context::new(config)?
        })
    }

    pub fn store(&self) -> Result<(), Error> {
        let context_json = ContextJson {
            history: self.history.iter().map(
                |h| h.id.clone()
            ).collect(),
            curr_raw_response: self.curr_raw_response.clone(),
            user_request: self.user_request.clone(),
            completed_user_requests: self.completed_user_requests.clone(),
        };

        Ok(write_string(
            &join(".neukgu", "context.json")?,
            &serde_json::to_string_pretty(&context_json)?,
            WriteMode::Atomic,
        )?)
    }

    pub fn start_turn(&mut self, raw_response: String, llm_elapsed_ms: u64) {
        assert!(self.curr_raw_response.is_none());
        self.curr_raw_response = Some((raw_response, llm_elapsed_ms));
    }

    pub fn finish_turn(
        &mut self,
        parse_result: Option<Vec<ParsedSegment>>,
        turn_result: TurnResult,
        tool_elapsed_ms: u64,
        config: &Config,
        is_fake_turn: bool,
    ) -> Result<(), Error> {
        let (raw_response, llm_elapsed_ms) = self.curr_raw_response.take().unwrap();
        let new_turn = Turn::new(
            raw_response,
            parse_result,
            turn_result,
            llm_elapsed_ms,
            tool_elapsed_ms,
            is_fake_turn,
            config,
        );
        let new_turn_summary = new_turn.summary(config);
        new_turn.store()?;
        self.history.push(new_turn_summary.clone());

        if let TurnResult::ToolCallSuccess(ToolCallSuccess::Write { path, .. }) = &new_turn.turn_result {
            self.logger.log_file_content(path, &new_turn.id)?;
        }

        Ok(())
    }

    pub fn is_reading_too_much(&mut self) -> Result<bool, Error> {
        Ok(self.history.len() >= 5 && {
            let this_turn = self.history.last().unwrap();

            this_turn.result == TurnResultSummary::ToolCallSuccess && {
                let recent_5_turn_ids = self.history.iter().rev().filter(
                    |t| t.result != TurnResultSummary::ParseError
                ).take(5).map(|t| t.clone()).collect::<Vec<_>>();
                let mut recent_5_turns = vec![];

                for turn_id in recent_5_turn_ids.iter() {
                    recent_5_turns.push(self.load_turn(&turn_id.id)?);
                }

                recent_5_turns.iter().all(
                    |turn| matches!(
                        turn.turn_result,
                        TurnResult::ToolCallSuccess(ToolCallSuccess::ReadText { .. } | ToolCallSuccess::ReadImage { .. } | ToolCallSuccess::ReadDir { .. }),
                    )
                )
            }
        })
    }

    pub fn load_turn(&mut self, id: &TurnId) -> Result<Turn, Error> {
        if let Some(turn) = self.turns.get(&id) {
            return Ok(turn.clone());
        }

        let turn = Turn::load(id)?;
        self.turns.insert(id.clone(), turn.clone());
        Ok(turn.clone())
    }

    pub fn to_request(&mut self, config: &Config) -> Result<Request, Error> {
        assert!(self.curr_raw_response.is_none());
        let (history, query) = self.fit_history_to_llm_context(config)?;

        Ok(Request {
            model: config.model()?,
            system_prompt: self.system_prompt.to_string(),
            history,
            query,
            enable_web_search: false,

            // When enabled,
            thinking: Thinking::Disabled,
        })
    }

    // This is the core of context engineering.
    // 1. If the LLM context is too long, the LLM will degrade.
    // 2. If we omit/summary too much information the LLM will degrade.
    //
    // # Strategies
    //
    // 1. For `request::Turn.response`, we can either use `Turn.raw_response` or tool-call xml of `Turn.parse_result`.
    //   - `Turn.raw_response` has more information, but is longer.
    //   - I'll call it full-render and short-render.
    // 2. If there's `TurnResult::ParseError`, it's likely that the LLM made a basic mistake (e.g. wrong param name),
    //    and likely tried the same tool-call with correct syntax. In this case, we can omit the mistaken turns.
    // 3. If there's `TurnResult::ToolCallError`, it's also likely that the LLM made a mistake and tried a similar
    //    tool-call again. It has more information than `TurnResult::ParseError`, though.
    // 4. If there are too many turns, we have to omit less important turns. But how do we know which turn is important?
    //    - Recent turns are likely to be more relevant than old turns.
    //    - The LLM is likely to gather important information in early turns (e.g. reading `instruction.md`).
    fn fit_history_to_llm_context(&mut self, config: &Config) -> Result<(Vec<request::Turn>, Vec<LLMToken>), Error> {
        let mut truncated_context = false;

        let chosen_turns = 'b: {
            // Candidate 1: Full-render every turn.
            let candidate: Vec<(TurnSummary, bool)> = self.history.iter().map(|s| (s.clone(), true)).collect();

            // 1. If there are less than or equal to 5 turns, it full-renders everything.
            //    - In this case, it doesn't check max_len.
            // 2. If full-rendering every turns fits in max_len, it full-renders everything.
            if self.history.len() <= 5 || count_llm_context_len(&candidate) < config.llm_context_max_len {
                break 'b candidate;
            }

            truncated_context = true;

            // Candidate 2: Full-render the last 2 turns and short-render the other turns.
            let mut candidate: Vec<(TurnSummary, bool)> = self.history[..(self.history.len() - 2)].iter().map(|s| (s.clone(), false)).collect();
            candidate.push((self.history[self.history.len() - 2].clone(), true));
            candidate.push((self.history[self.history.len() - 1].clone(), true));

            if count_llm_context_len(&candidate) < config.llm_context_max_len {
                break 'b candidate;
            }

            // Candidate 3: Full-render the last 2 turns. Filter out pasre-error turns in the other turns and short-render them.
            let mut candidate: Vec<(TurnSummary, bool)> = self.history[..(self.history.len() - 2)].iter().filter(
                |s| s.result != TurnResultSummary::ParseError
            ).map(
                |s| (s.clone(), false)
            ).collect();
            candidate.push((self.history[self.history.len() - 2].clone(), true));
            candidate.push((self.history[self.history.len() - 1].clone(), true));

            if count_llm_context_len(&candidate) < config.llm_context_max_len {
                break 'b candidate;
            }

            // We have to omit some turns...
            // My guess here is that
            //    1. Recent turns are more important than old turns.
            //    2. Very early turns are important, because `instruction.md` is very likely to be there.
            // So I fill the first quarter with the very first turns and the remaining 3 quarters with the recent turns.
            //
            // It doesn't include parse-error turns.
            let mut pre_len = config.llm_context_max_len / 4;
            let mut pre_turns = vec![];
            let mut post_len = config.llm_context_max_len * 3 / 4;
            let mut post_turns = vec![];

            // TODO: What if short-rendered first turn is longer than pre_len?
            for turn in self.history.iter() {
                if turn.llm_len_short > pre_len {
                    break;
                }

                if turn.result != TurnResultSummary::ParseError {
                    pre_turns.push((turn.clone(), false));
                    pre_len -= turn.llm_len_short;
                }
            }

            post_len += pre_len;

            for (i, turn) in self.history.iter().rev().enumerate() {
                // The most recent 2 turns are always full-rendered.
                if i < 2 {
                    post_turns.push((turn.clone(), true));
                    post_len = post_len.max(turn.llm_len_full) - turn.llm_len_full;
                    continue;
                }

                if turn.llm_len_short > post_len {
                    break;
                }

                if turn.result != TurnResultSummary::ParseError {
                    post_turns.push((turn.clone(), false));
                    post_len -= turn.llm_len_short;
                }
            }

            let chosen_turns = vec![
                pre_turns,
                post_turns.into_iter().rev().collect(),
            ].concat();
            break 'b chosen_turns;
        };

        if truncated_context {
            self.logger.log(LogEntry::TruncatedContext(chosen_turns.iter().map(
                |(turn, full_render)| ChosenTurn { turn: turn.id.clone(), full_render: *full_render }
            ).collect()))?;
        }

        let mut llm_turns = vec![request::Turn {
            // TODO: better starting message?
            query: vec![LLMToken::String(String::from("Go on."))],
            response: String::new(),
        }];

        for (turn, full_render) in chosen_turns.iter() {
            let turn = self.load_turn(&turn.id)?;
            llm_turns.last_mut().unwrap().response = if *full_render {
                turn.raw_response.to_string()
            } else {
                if let Some(parse_result) = &turn.parse_result {
                    let Some(ParsedSegment::ToolCall { input, .. }) = get_first_tool_call(parse_result) else { unreachable!() };
                    input.to_string()
                }

                else {
                    turn.raw_response.to_string()
                }
            };
            llm_turns.push(request::Turn {
                query: turn.turn_result.to_llm_tokens(config),
                response: String::new(),
            });
        }

        let query = llm_turns.pop().unwrap().query;
        Ok((llm_turns, query))
    }
}

fn count_llm_context_len(turns: &[(TurnSummary, bool)]) -> u64 {
    turns.iter().map(
        |(turn, full_render)| if *full_render {
            turn.llm_len_full
        } else {
            turn.llm_len_short
        }
    ).sum()
}

// for logging
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChosenTurn {
    pub turn: TurnId,
    pub full_render: bool,
}
