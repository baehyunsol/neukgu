use crate::{
    AskTo,
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
    revert_mock_state,
    validate_parse_result,
};
use ragit_fs::{WriteMode, exists, join3, read_string, remove_file, write_string};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub struct Context {
    // You'll find the index dir at `<working_dir>/.neukgu/`
    pub working_dir: String,

    pub history: Vec<TurnSummary>,

    // If we have this, that means we already have LLM's response,
    // so we just have to run tool-call (or throw a parse error).
    pub curr_raw_response: Option<(String, u64)>,

    pub completed_questions_from_user: HashSet<u64>,
    pub hidden_turns: HashSet<TurnId>,
    pub pinned_turns: HashSet<TurnId>,  // never hidden

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
    pub completed_questions_from_user: HashSet<u64>,
    pub hidden_turns: HashSet<TurnId>,
    pub pinned_turns: HashSet<TurnId>,
}

impl Context {
    pub fn new(config: &Config, working_dir: &str) -> Result<Self, Error> {
        let system_prompt = tera::Tera::one_off(
            include_str!("../system.pdl"),
            &config.system_prompt_context(),
            true,
        )?;
        let available_binaries = load_available_binaries(working_dir)?;
        let logger = Logger::new(&join3(working_dir, ".neukgu", "logs")?);

        Ok(Context {
            working_dir: working_dir.to_string(),
            history: vec![],
            curr_raw_response: None,
            turns: HashMap::new(),
            completed_questions_from_user: HashSet::new(),
            hidden_turns: HashSet::new(),
            pinned_turns: HashSet::new(),
            system_prompt,
            available_binaries,
            logger,
        })
    }

    pub fn load(config: &Config, working_dir: &str) -> Result<Self, Error> {
        let s = read_string(&join3(working_dir, ".neukgu", "context.json")?)?;
        let context_json: ContextJson = serde_json::from_str(&s)?;

        Ok(Context {
            working_dir: working_dir.to_string(),
            history: context_json.history.iter().map(
                |h| h.get_turn_summary()
            ).collect(),
            curr_raw_response: context_json.curr_raw_response.clone(),
            completed_questions_from_user: context_json.completed_questions_from_user.clone(),
            hidden_turns: context_json.hidden_turns.clone(),
            pinned_turns: context_json.pinned_turns.clone(),
            ..Context::new(config, working_dir)?
        })
    }

    pub fn store(&self) -> Result<(), Error> {
        let context_json = ContextJson {
            history: self.history.iter().map(
                |h| h.id.clone()
            ).collect(),
            curr_raw_response: self.curr_raw_response.clone(),
            completed_questions_from_user: self.completed_questions_from_user.clone(),
            hidden_turns: self.hidden_turns.clone(),
            pinned_turns: self.pinned_turns.clone(),
        };

        Ok(write_string(
            &join3(&self.working_dir, ".neukgu", "context.json")?,
            &serde_json::to_string_pretty(&context_json)?,
            WriteMode::Atomic,
        )?)
    }

    pub fn start_turn(&mut self, raw_response: String, llm_elapsed_ms: u64) {
        assert!(self.curr_raw_response.is_none());
        self.curr_raw_response = Some((raw_response, llm_elapsed_ms));
    }

    pub fn discard_current_turn(&mut self) {
        assert!(self.curr_raw_response.is_some());
        self.curr_raw_response = None;
    }

    pub fn finish_turn(
        &mut self,
        parse_result: Option<Vec<ParsedSegment>>,
        turn_result: TurnResult,
        tool_elapsed_ms: u64,
        config: &Config,
        is_question_from_user: bool,
    ) -> Result<(), Error> {
        let (raw_response, llm_elapsed_ms) = self.curr_raw_response.take().unwrap();
        let new_turn = Turn::new(
            raw_response,
            parse_result,
            turn_result,
            llm_elapsed_ms,
            tool_elapsed_ms,
            is_question_from_user,
            config,
        );
        let new_turn_summary = new_turn.summary(config);
        new_turn.store(&self.working_dir)?;
        self.history.push(new_turn_summary.clone());
        Ok(())
    }

    pub fn discard_previous_turn(&mut self) {
        assert!(self.curr_raw_response.is_none());
        revert_mock_state(&self.working_dir).unwrap();
        self.history.pop().unwrap();
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
                        TurnResult::ToolCallSuccess(
                            ToolCallSuccess::ReadText { .. } |
                            ToolCallSuccess::ReadPdf { .. } |
                            ToolCallSuccess::ReadImage { .. } |
                            ToolCallSuccess::ReadDir { .. }
                        ),
                    )
                )
            }
        })
    }

    pub fn load_turn(&mut self, id: &TurnId) -> Result<Turn, Error> {
        if let Some(turn) = self.turns.get(&id) {
            return Ok(turn.clone());
        }

        let turn = Turn::load(id, &self.working_dir)?;
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
    //    - The LLM is likely to gather important information in early turns (e.g. reading `neukgu-instruction.md`).
    fn fit_history_to_llm_context(&mut self, config: &Config) -> Result<(Vec<request::Turn>, Vec<LLMToken>), Error> {
        let chosen_turns = 'b: {
            // Candidate 1: Full-render every turn.
            let candidate: Vec<(TurnSummary, bool)> = self.history.iter()
                .filter(|s| !self.hidden_turns.contains(&s.id))
                .map(|s| (s.clone(), true))
                .collect();

            // 1. If there are less than or equal to 5 turns, it full-renders everything.
            //    - In this case, it doesn't check max_len.
            // 2. If full-rendering every turns fits in max_len, it full-renders everything.
            if self.history.len() <= 5 || count_llm_context_len(&candidate) < config.llm_context_max_len {
                break 'b candidate;
            }

            // Candidate 2: Full-render the last 2 turns and short-render the other turns.
            let mut candidate: Vec<(TurnSummary, bool)> = self.history[..(self.history.len() - 2)]
                .iter()
                .filter(|s| !self.hidden_turns.contains(&s.id))
                .map(|s| (s.clone(), false))
                .collect();

            candidate.push((self.history[self.history.len() - 2].clone(), true));
            candidate.push((self.history[self.history.len() - 1].clone(), true));

            if count_llm_context_len(&candidate) < config.llm_context_max_len {
                break 'b candidate;
            }

            // Candidate 3: Full-render the last 2 turns. Filter out pasre-error turns in the other turns and short-render them.
            let mut candidate: Vec<(TurnSummary, bool)> = self.history[..(self.history.len() - 2)].iter().filter(
                |s| s.result != TurnResultSummary::ParseError && !self.hidden_turns.contains(&s.id)
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
            //    2. Very early turns are important, because `neukgu-instruction.md` is very likely to be there.
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

                if turn.result != TurnResultSummary::ParseError && !self.hidden_turns.contains(&turn.id) {
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

                if turn.result != TurnResultSummary::ParseError && !self.hidden_turns.contains(&turn.id) {
                    post_turns.push((turn.clone(), false));
                    post_len -= turn.llm_len_short;
                }
            }

            let pre_turns_set: HashSet<TurnId> = pre_turns.iter().map(|turn| turn.0.id.clone()).collect();
            let post_turns_set: HashSet<TurnId> = post_turns.iter().map(|turn| turn.0.id.clone()).collect();

            // pinned turns are short-rendered
            let pinned_turns: Vec<(TurnSummary, bool)> = self.history.iter().filter(
                |turn| self.pinned_turns.contains(&turn.id) && !pre_turns_set.contains(&turn.id) && !post_turns_set.contains(&turn.id)
            ).map(
                |turn| (turn.clone(), false)
            ).collect();

            let chosen_turns = vec![
                pre_turns,
                pinned_turns,
                post_turns.into_iter().rev().collect(),
            ].concat();
            break 'b chosen_turns;
        };

        self.logger.log(LogEntry::TruncatedContext(chosen_turns.iter().map(
            |(turn, full_render)| ChosenTurn { turn: turn.id.clone(), full_render: *full_render }
        ).collect()))?;

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

    pub fn process_question_from_user(&mut self, id: u64, interrupt: String, config: &Config) -> Result<(), Error> {
        let q = "
<ask>
<to>user</to>
<question>Do you have any feedbacks?</question>
</ask>
";
        // Let's make sure that the schema is correct.
        self.curr_raw_response = Some((q.to_string(), 0));
        let parse_result = crate::parse::parse(q.as_bytes()).unwrap();
        let _tool_call = validate_parse_result(&parse_result).unwrap();

        let turn_result = TurnResult::ToolCallSuccess(ToolCallSuccess::Ask { to: AskTo::User, answer: interrupt });
        self.finish_turn(
            Some(parse_result),
            turn_result,
            0,
            config,
            true,
        )?;
        self.completed_questions_from_user.insert(id);
        Ok(())
    }

    pub fn is_marked_done(&self) -> Result<bool, Error> {
        Ok(exists(&join3(&self.working_dir, "logs", "done")?))
    }

    pub fn remove_done_mark(&self) -> Result<(), Error> {
        let done_mark = join3(&self.working_dir, "logs", "done")?;

        if exists(&done_mark) {
            remove_file(&done_mark)?;
        }

        Ok(())
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
