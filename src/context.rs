use chrono::Local;
use crate::{
    ApiLog,
    AskTo,
    Config,
    Error,
    InterruptId,
    LLMToken,
    Logger,
    LogEntry,
    ParsedSegment,
    Request,
    Thinking,
    ToolCallSuccess,
    ToolKind,
    Turn,
    TurnId,
    TurnKind,
    TurnResult,
    TurnResultSummary,
    TurnSummary,
    UserAnswer,
    get_global_index_dir,
    hash_bytes,
    init_and_load_available_binaries,
    prettify_time,
    prompt,
    request,
    revert_mock_state,
    system_prompt,
};
use ragit_fs::{
    WriteMode,
    copy_file,
    exists,
    into_abs_path,
    join3,
    join4,
    normalize,
    read_string,
    remove_file,
    write_string,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct NeukguId(pub(crate) u64);

impl NeukguId {
    pub fn new() -> NeukguId {
        NeukguId(rand::random())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct SessionId(pub(crate) u64);

impl SessionId {
    pub fn new() -> SessionId {
        SessionId(rand::random())
    }

    pub fn from_string_hash(s: &str) -> SessionId {
        let hash = (hash_bytes(s.as_bytes()) & 0xffff_ffff_ffff_ffff) as u64;
        SessionId(hash)
    }
}

pub struct Context {
    pub instruction: String,

    // If it's a sub-agent, the AI will give the name.
    // Otherwise, the user can give the name, so that it's easier to browse history.
    // The name matches `session_id`, not `neukgu_id`.
    pub name: Option<String>,

    // `neukgu_id` is created when `.neukgu/` is created and never changes.
    // `session_id` is updated when `.neukgu/` is created or the session is reset (manual or sub-agent).
    pub neukgu_id: NeukguId,
    pub session_id: SessionId,

    // If it has a parent, when the session is complete (logs/done is created),
    // the session is automatically switched to the parent.
    pub parent: Option<SessionId>,

    // You'll find the index dir at `<working_dir>/.neukgu/`
    pub working_dir: String,

    pub history: Vec<TurnSummary>,
    pub summaries: Vec<TurnId>,

    // If we have this, that means we already have LLM's response,
    // so we just have to run tool-call (or throw a parse error).
    pub curr_raw_response: Option<RawResponse>,

    pub completed_interrupts_from_user: HashSet<InterruptId>,
    pub hidden_turns: HashSet<TurnId>,
    pub pinned_turns: HashSet<TurnId>,  // never hidden
    pub is_in_global_index_dir: bool,
    pub has_to_remove_done_mark: bool,

    // After the agent marks `neukgu-logs/done`, a small agent will write a final report
    // that summaries the session.
    pub final_report: FinalReport,

    pub updated_at: i64,

    // in-memory data structures
    pub turns: HashMap<TurnId, Turn>,  // it's lazily loaded
    pub available_binaries: Vec<String>,
    pub global_index_dir: String,
    pub logger: Logger,

    // If FE wants to update the config, this field will be set.
    pub new_config: Option<Config>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ContextJson {
    pub name: Option<String>,
    pub instruction: String,
    pub neukgu_id: NeukguId,
    pub session_id: SessionId,
    pub parent: Option<SessionId>,
    pub history: Vec<TurnId>,
    pub summaries: Vec<TurnId>,
    pub curr_raw_response: Option<RawResponse>,
    pub completed_interrupts_from_user: HashSet<InterruptId>,
    pub hidden_turns: HashSet<TurnId>,
    pub pinned_turns: HashSet<TurnId>,
    pub is_in_global_index_dir: bool,
    pub has_to_remove_done_mark: bool,
    pub final_report: FinalReport,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RawResponse {
    pub thinking: Option<String>,
    pub response: String,
    pub elapsed_ms: u64,
    pub logs: ApiLog,
}

impl Context {
    pub fn new(
        working_dir: &str,
        name: Option<String>,
        instruction: String,

        // If not set, a random id is given
        neukgu_id: Option<NeukguId>,
        session_id: Option<SessionId>,

        is_in_global_index_dir: bool,
        parent: Option<SessionId>,
    ) -> Result<Self, Error> {
        let available_binaries = init_and_load_available_binaries(working_dir)?;
        let global_index_dir = get_global_index_dir()?;
        let logger = Logger::new(
            join3(working_dir, ".neukgu", "logs")?,
            Some(join4(&global_index_dir, "project-logger", ".neukgu", "logs")?),
            true,
            true,
        );

        Ok(Context {
            name,
            instruction,
            neukgu_id: neukgu_id.unwrap_or_else(|| NeukguId::new()),
            session_id: session_id.unwrap_or_else(|| SessionId::new()),
            parent,
            working_dir: normalize(&into_abs_path(working_dir)?)?,
            history: vec![],
            summaries: vec![],
            curr_raw_response: None,
            turns: HashMap::new(),
            completed_interrupts_from_user: HashSet::new(),
            hidden_turns: HashSet::new(),
            pinned_turns: HashSet::new(),
            is_in_global_index_dir,
            available_binaries,
            global_index_dir,
            has_to_remove_done_mark: false,
            final_report: FinalReport::SessionNotDone,
            updated_at: Local::now().timestamp_millis(),
            logger,
            new_config: None,
        })
    }

    pub fn from_session_id(id: SessionId, working_dir: &str) -> Result<Self, Error> {
        Context::load_worker(&join4(working_dir, ".neukgu", "sessions", &format!("{:016x}.json", id.0))?, working_dir)
    }

    pub fn load(working_dir: &str) -> Result<Self, Error> {
        Context::load_worker(&join3(working_dir, ".neukgu", "context.json")?, working_dir)
    }

    fn load_worker(json_at: &str, working_dir: &str) -> Result<Self, Error> {
        let s = read_string(json_at)?;
        let context_json: ContextJson = serde_json::from_str(&s)?;

        Ok(Context {
            name: context_json.name.clone(),
            instruction: context_json.instruction.to_string(),
            neukgu_id: context_json.neukgu_id,
            session_id: context_json.session_id,
            parent: context_json.parent.clone(),
            working_dir: normalize(&into_abs_path(working_dir)?)?,
            history: context_json.history.iter().map(
                |h| h.get_turn_summary()
            ).collect(),
            summaries: context_json.summaries.clone(),
            curr_raw_response: context_json.curr_raw_response.clone(),
            completed_interrupts_from_user: context_json.completed_interrupts_from_user.clone(),
            hidden_turns: context_json.hidden_turns.clone(),
            pinned_turns: context_json.pinned_turns.clone(),
            is_in_global_index_dir: context_json.is_in_global_index_dir,
            has_to_remove_done_mark: context_json.has_to_remove_done_mark,
            final_report: context_json.final_report.clone(),
            updated_at: context_json.updated_at,
            ..Context::new(working_dir, None, String::new(), None, None, false, None)?
        })
    }

    // It's stored in 2 places.
    // 1. `.neukgu/context.json` -> it represents the current session.
    // 2. `.neukgu/sessions/<session-id>.json` -> it stores every sessions.
    pub fn store(&self) -> Result<(), Error> {
        write_string(
            &join3(&self.working_dir, ".neukgu", "context.json")?,
            &serde_json::to_string_pretty(&self.to_json())?,
            WriteMode::Atomic,
        )?;
        copy_file(
            &join3(&self.working_dir, ".neukgu", "context.json")?,
            &join4(&self.working_dir, ".neukgu", "sessions", &format!("{:016x}.json", self.session_id.0))?,
        )?;

        Ok(())
    }

    pub(crate) fn to_json(&self) -> ContextJson {
        ContextJson {
            name: self.name.clone(),
            instruction: self.instruction.to_string(),
            neukgu_id: self.neukgu_id,
            session_id: self.session_id,
            parent: self.parent.clone(),
            history: self.history.iter().map(
                |h| h.id.clone()
            ).collect(),
            summaries: self.summaries.clone(),
            curr_raw_response: self.curr_raw_response.clone(),
            completed_interrupts_from_user: self.completed_interrupts_from_user.clone(),
            hidden_turns: self.hidden_turns.clone(),
            pinned_turns: self.pinned_turns.clone(),
            is_in_global_index_dir: self.is_in_global_index_dir,
            has_to_remove_done_mark: self.has_to_remove_done_mark,
            final_report: self.final_report.clone(),
            updated_at: self.updated_at,
        }
    }

    pub fn remove_done_mark(&self) -> Result<(), Error> {
        remove_file(&join3(&self.working_dir, "neukgu-logs", "done")?)?;
        Ok(())
    }

    pub async fn try_write_final_report(&mut self, config: &Config) -> Result<(), Error> {
        if self.final_report != FinalReport::SessionNotDone {
            return Ok(());
        }

        self.logger.log(LogEntry::WriteFinalReportStart)?;
        let (mut history, mut last_turn) = self.get_context_llm_tokens(config)?;
        last_turn.push(LLMToken::String(String::from("\n\nNow I want you to write the final report. Before writing the report, I'll give you a brief summary of the current session. Use this informatio if necessary.")));
        history.push(request::Turn {
            query: last_turn,
            response: String::from("Okay, give me the summary, then I'll write the final report"),
        });
        let request = Request {
            model: config.agents.small,
            system_prompt: prompt::final_report_system_prompt(),
            history,
            query: vec![LLMToken::String(self.summary_for_final_report(config)?)],
            enable_web_search: false,
            thinking: Thinking::Disabled,
        };

        match request.request(&config.request_config(), &self.working_dir, true, &self.logger).await {
            Ok(response) => {
                self.final_report = FinalReport::Report(response.response);
            },
            Err(e) => {
                self.final_report = FinalReport::Error(format!("{e:?}"));
            },
        }

        self.logger.log(LogEntry::WriteFinalReportEnd)?;
        Ok(())
    }

    pub fn start_turn(
        &mut self,
        thinking: Option<String>,
        response: String,
        elapsed_ms: u64,
        logs: ApiLog,
    ) {
        assert!(self.curr_raw_response.is_none());
        self.curr_raw_response = Some(RawResponse { thinking, response, elapsed_ms, logs });
    }

    pub fn discard_current_turn(&mut self) {
        assert!(self.curr_raw_response.is_some());
        self.curr_raw_response = None;
    }

    pub fn finish_turn(
        &mut self,
        parse_result: Option<ParsedSegment>,
        turn_result: TurnResult,
        tool_elapsed_ms: u64,
        config: &Config,
        kind: TurnKind,
    ) -> Result<TurnId, Error> {
        let RawResponse { thinking, response, elapsed_ms, logs } = self.curr_raw_response.take().unwrap();
        let new_turn = Turn::new(
            thinking,
            response,
            parse_result,
            turn_result,
            elapsed_ms,
            tool_elapsed_ms,
            kind,
            config,
            logs,
        );
        let new_turn_summary = new_turn.summary(config);
        new_turn.store(&self.working_dir)?;
        let new_turn_id = new_turn_summary.id.clone();
        self.history.push(new_turn_summary);
        Ok(new_turn_id)
    }

    pub fn discard_previous_turn(&mut self) {
        assert!(self.curr_raw_response.is_none());
        revert_mock_state(&self.working_dir).unwrap();
        self.history.pop().unwrap();
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
        let (history, query) = self.get_context_llm_tokens(config)?;

        Ok(Request {
            model: config.agents.big,
            system_prompt: system_prompt(config, &self.working_dir),
            history,
            query,
            enable_web_search: false,

            // When enabled,
            thinking: Thinking::Disabled,
        })
    }

    // The harness may inject *fake* turns to nudge the behavior of the LLM.
    //
    // # 1. Start of a session
    //
    // The first turn is always `<read><path>.</path></read>` and the second turn
    // is always `<ask><to>user</to><question>What do you want me to do?</question></ask>`.
    //
    // It's beneficial in 2 ways:
    //
    // 1. It's a few-shot example for the LLM that shows how to call tools.
    // 2. The LLM is gonna do this anyway, so we can save time and cost by skipping
    //    the LLM api.
    //
    // # 2. Remove `logs/done`
    pub fn get_fake_turn(&mut self) -> Option<(String, TurnKind, Option<TurnResult>)> {
        let session_starts: Vec<&TurnSummary> = self.history.iter().filter(
            |turn| turn.kind == TurnKind::SessionStart
        ).collect();

        match session_starts.len() {
            0 => return Some((
                String::from(
"Let's first check what I have in the working directory.
<read>
<path>.</path>
</read>
"
                ),
                TurnKind::SessionStart,
                None,
            )),
            1 => {
                let turn_result = TurnResult::ToolCallSuccess(ToolCallSuccess::Ask { to: AskTo::User, answer: UserAnswer::FreeText(self.instruction.to_string()) });

                match session_starts[0].result {
                    TurnResultSummary::ParseError => unreachable!(),
                    TurnResultSummary::ToolCallError => return Some((
                        String::from(
"Okay, I can't get the list of files in the working directory. Let me just ask what the user wants.
<ask>
<to>user</to>
<question>What do you want me to do?</question>
</ask>
"
                        ),
                        TurnKind::SessionStart,
                        Some(turn_result),
                    )),
                    TurnResultSummary::ToolCallSuccess => return Some((
                        String::from(
"Okay, I see the files. Let me ask what the user wants.
<ask>
<to>user</to>
<question>What do you want me to do?</question>
</ask>
"
                        ),
                        TurnKind::SessionStart,
                        Some(turn_result),
                    )),
                }
            },
            _ => {},
        }

        if self.has_to_remove_done_mark {
            self.has_to_remove_done_mark = false;
            Some((
                String::from("
Let me remove `neukgu-logs/done` and continue working.
<remove>
<path>neukgu-logs/done</path>
</remove>
                "),
                TurnKind::RemoveDoneMark,
                None,
            ))
        }

        else {
            None
        }
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
    //    - The second turn contains the instruction, which is very important.
    fn fit_history_to_context(&mut self, config: &Config) -> Vec<(TurnSummary, bool)> {
        // NOTE: User questions never go into the context.

        // Candidate 1: Full-render every turn.
        let candidate: Vec<(TurnSummary, bool)> = self.history.iter()
            .filter(|s| !self.hidden_turns.contains(&s.id) && s.kind != TurnKind::UserQuestion)
            .map(|s| (s.clone(), true))
            .collect();

        // 1. If there are less than or equal to 5 turns, it full-renders everything.
        //    - In this case, it doesn't check max_len.
        // 2. If full-rendering every turns fits in max_len, it full-renders everything.
        if self.history.len() <= 5 || count_llm_context_len(&candidate) < config.llm_context_max_len {
            return candidate;
        }

        // Candidate 2: Full-render the last 2 turns and short-render the other turns.
        let mut candidate: Vec<(TurnSummary, bool)> = self.history[..(self.history.len() - 2)]
            .iter()
            .filter(|s| !self.hidden_turns.contains(&s.id) && s.kind != TurnKind::UserQuestion)
            .map(|s| (s.clone(), false))
            .collect();

        candidate.push((self.history[self.history.len() - 2].clone(), true));
        candidate.push((self.history[self.history.len() - 1].clone(), true));

        if count_llm_context_len(&candidate) < config.llm_context_max_len {
            return candidate;
        }

        // Candidate 3: Full-render the last 2 turns. Filter out pasre-error turns in the other turns and short-render them.
        let mut candidate: Vec<(TurnSummary, bool)> = self.history[..(self.history.len() - 2)].iter().filter(
            |s| s.result != TurnResultSummary::ParseError && !self.hidden_turns.contains(&s.id) && s.kind != TurnKind::UserQuestion
        ).map(
            |s| (s.clone(), false)
        ).collect();

        candidate.push((self.history[self.history.len() - 2].clone(), true));
        candidate.push((self.history[self.history.len() - 1].clone(), true));

        if count_llm_context_len(&candidate) < config.llm_context_max_len {
            return candidate;
        }

        // We have to omit some turns...
        // My guess here is that
        //    1. Recent turns are more important than old turns.
        //    2. Very early turns are important, because it has the user instruction!
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

            if turn.result != TurnResultSummary::ParseError && !self.hidden_turns.contains(&turn.id) && turn.kind != TurnKind::UserQuestion {
                pre_turns.push((turn.clone(), false));
                pre_len -= turn.llm_len_short;
            }
        }

        post_len += pre_len;
        let mut most_recent_2_turns = 2;

        for turn in self.history.iter().rev() {
            // The most recent 2 turns are always full-rendered.
            if most_recent_2_turns > 0 {
                if turn.kind != TurnKind::UserQuestion {
                    post_turns.push((turn.clone(), true));
                    post_len = post_len.max(turn.llm_len_full) - turn.llm_len_full;
                    most_recent_2_turns -= 1;
                }

                continue;
            }

            if turn.llm_len_short > post_len {
                break;
            }

            if turn.result != TurnResultSummary::ParseError && !self.hidden_turns.contains(&turn.id) && turn.kind != TurnKind::UserQuestion {
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

        vec![
            pre_turns,
            pinned_turns,
            post_turns.into_iter().rev().collect(),
        ].concat()
    }

    pub(crate) fn get_context_llm_tokens(&mut self, config: &Config) -> Result<(Vec<request::Turn>, Vec<LLMToken>), Error> {
        let chosen_turns = self.fit_history_to_context(config);
        self.logger.log(LogEntry::TruncatedContext(chosen_turns.iter().map(
            |(turn, full_render)| ChosenTurn { turn: turn.id.clone(), full_render: *full_render }
        ).collect()))?;

        let mut llm_turns = vec![request::Turn {
            query: vec![LLMToken::String(String::from("Go on."))],
            response: String::new(),
        }];

        for (turn, full_render) in chosen_turns.iter() {
            let turn = self.load_turn(&turn.id)?;
            llm_turns.last_mut().unwrap().response = turn.render_llm_response(*full_render);
            llm_turns.push(request::Turn {
                query: turn.turn_result.to_llm_tokens(config),
                response: String::new(),
            });
        }

        let query = llm_turns.pop().unwrap().query;
        Ok((llm_turns, query))
    }

    pub fn has_done_mark(&self) -> bool {
        let done_mark_at = &join3(&self.working_dir, "neukgu-logs", "done").unwrap();
        !self.has_to_remove_done_mark && exists(&done_mark_at)
    }

    // 1. The number of tool-calls: visible in context vs all
    // 2. write/edited files: logs / others
    pub(crate) fn summary_for_final_report(&mut self, config: &Config) -> Result<String, Error> {
        let all_turns = self.history.to_vec();
        let tool_calls = all_turns.iter().filter(|t| t.result == TurnResultSummary::ToolCallSuccess).count();
        let failed_tool_calls = all_turns.len() - tool_calls;
        let visible_turns: Vec<TurnSummary> = self.fit_history_to_context(config).into_iter().map(|(t, _)| t).collect();
        let visible_tool_calls = visible_turns.iter().filter(|t| t.result == TurnResultSummary::ToolCallSuccess).count();

        let tool_call_summary = {
            struct ToolCallSummary {
                elapsed_ms: u64,
                succ: usize,
                fail: usize,
            }

            let mut result: HashMap<ToolKind, ToolCallSummary> = ToolKind::all().iter().map(
                |tool| (*tool, ToolCallSummary { elapsed_ms: 0, succ: 0, fail: 0 })
            ).collect();

            for turn in all_turns.iter() {
                let turn = self.load_turn(&turn.id)?;

                if let Some(ParsedSegment { tool: Some(tool), .. }) = &turn.parse_result {
                    let succ = matches!(turn.turn_result, TurnResult::ToolCallSuccess(_));
                    let kind = tool.kind();
                    let summary = result.get_mut(&kind).unwrap();
                    summary.elapsed_ms += turn.tool_elapsed_ms;

                    if succ {
                        summary.succ += 1;
                    } else {
                        summary.fail += 1;
                    }
                }
            }

            let result: Vec<(ToolKind, ToolCallSummary)> = ToolKind::all().iter().map(
                |k| (*k, result.remove(k).unwrap())
            ).collect();
            format!(
                "Tool call summary (some tools may not be activated in this session. Do not mention the unused tools in the final report)\n{}",
                result.iter().map(
                    |(kind, summary)| format!(
                        "- {}\n  - called {} time{} (success: {}, fail: {})\n  - total elapsed time: {}",
                        kind.tag_name(),
                        summary.succ + summary.fail,
                        if summary.succ + summary.fail == 1 { "" } else { "s" },
                        summary.succ,
                        summary.fail,
                        prettify_time(summary.elapsed_ms),
                    )
                ).collect::<Vec<_>>().join("\n"),
            )
        };

        Ok(format!("
Total tool calls: {tool_calls} ({visible_tool_calls} tool calls visible in context, {failed_tool_calls} failed tool call{})
{tool_call_summary}
",
            if failed_tool_calls == 1 { "" } else { "s" },
        ))
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

// When the agent writes `neukgu-logs/summary-XXX.md`, the context will remember the turn id.
// We can get `SessionSummary` from the turn.
#[derive(Clone, Debug)]
pub struct SessionSummary {
    pub timestamp: String,
    pub timestamp_millis: i64,
    pub title: String,
    pub summary: String,
}

// It's used by GUI.
#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub id: SessionId,
    pub name: Option<String>,
    pub instruction: String,
    pub updated_at: i64,
    pub selected: bool,
    pub finished: bool,
    pub parent: Option<SessionId>,
    pub sub_agents: Vec<SessionInfo>,
}

impl SessionInfo {
    pub fn sort_children(&mut self) {
        self.sub_agents.sort_by_key(|session| -session.updated_at);

        for child in self.sub_agents.iter_mut() {
            child.sort_children();
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum FinalReport {
    SessionNotDone,
    Report(String),
    Error(String),
}

impl FinalReport {
    pub fn is_finished(&self) -> bool {
        self != &FinalReport::SessionNotDone
    }
}
