use chrono::Local;
use crate::{
    ChosenTurn,
    Config,
    Context,
    ContextJson,
    Error,
    FileContent,
    LogId,
    TokenUsage,
    ToolCall,
    ToolCallSuccess,
    Turn,
    TurnId,
    TurnPreview,
    TurnResult,
    TurnSummary,
    WriteMode as ToolWriteMode,
    load_json,
    load_log,
    load_logs_tail,
    prettify_bytes,
    prettify_tokens,
};
use ragit_fs::{
    FileError,
    WriteMode as FsWriteMode,
    exists,
    into_abs_path,
    join3,
    join4,
    write_string,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use similar::Algorithm as DiffAlgorithm;
use similar::udiff::unified_diff;
use std::collections::HashMap;
use std::fs::TryLockError;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub mod tui;
pub mod gui;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Message {
    pub id: u64,
    pub kind: MessageKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MessageKind {
    LLM2User {
        question: String,
        answer: Option<UserResponse>,
    },
    User2LLM {
        question: String,
        completed: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum UserResponse {
    Answer(String),
    Timeout,
    Reject,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Be2Fe {
    // `Be2Fe` frequently reads `Fe2Be::pause` and updates this field.
    pub pause: bool,

    // `Fe2Be` frequently reads this field and updates `Fe2Be::from_be`.
    // When a message is pushed to `Fe2Be::from_be`, the message in this field will be removed.
    pub to_fe: HashMap<u64, Message>,

    // `Be2Fe` frequently reads `Fe2Be::to_be` and updates this field.
    pub from_fe: HashMap<u64, Message>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Fe2Be {
    // `Fe2Be` doesn't read `Be2Fe::pause`. It's set by user input.
    pub pause: bool,

    // `Be2Fe` frequently reads this field and updates `Be2Fe::from_fe`.
    // When a message is pushed to `Be2Fe::from_fe`, the message in this field will be removed.
    pub to_be: HashMap<u64, Message>,

    // `Fe2Be` frequently reads `Be2Fe::to_fe` and updates this field.
    pub from_be: HashMap<u64, Message>,

    // Fe will update this field once a second.
    pub updated_at: i64,
}

impl Default for Be2Fe {
    fn default() -> Be2Fe {
        Be2Fe {
            pause: false,
            to_fe: HashMap::new(),
            from_fe: HashMap::new(),
        }
    }
}

impl Default for Fe2Be {
    fn default() -> Fe2Be {
        Fe2Be {
            pause: false,
            to_be: HashMap::new(),
            from_be: HashMap::new(),
            updated_at: -1,
        }
    }
}

// In order to implement a new frontend, use this struct.
// The architecture should look like
// ```rs
// Command::new("neukgu").args(["headless", "--attach-fe"]).spawn()?;
// let mut context = FeContext::load()?;
//
// loop {
//     context.start_frame()?;
//     render(&context);
//     context.end_frame(interrupt, response)?;
// }
// ```
//
// The ui is supposed to call `end_frame` at least once per 5 seconds.
#[derive(Clone, Debug)]
pub struct FeContext {
    pub working_dir: String,
    pub history: Vec<TurnSummary>,
    pub curr_tool_call: Option<ToolCall>,
    pub config: Config,
    pub initialized_at: Instant,

    // If it experienced an API error (status code not 200..300) in
    // the current turn, the status code is recorded here.
    pub last_api_error: Option<u16>,

    pub last_backend_error: Option<String>,

    // It indicates whether a turn is hidden, full-rendered or short-rendered in the LLM context.
    pub truncation: HashMap<TurnId, Truncation>,

    pub previews: HashMap<TurnId, TurnPreview>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Truncation {
    Hidden,
    FullRender,
    ShortRender,
}

pub static INIT_LOGGER_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[.+\].+init_logger.*").unwrap());
pub static TRUNCATED_CONTEXT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[.+\].+truncated_context\((\d+\-\d+)\).*").unwrap());
pub static GOT_RESPONSE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[.+\].+got_response\((\d+)\).*").unwrap());
pub static TOOL_CALL_START_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[.+\].+tool_call_start\((\d+\-\d+)\).*").unwrap());
pub static TOOL_CALL_END_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[.+\].+tool_call_end\((\d+\-\d+)\).*").unwrap());
pub static BACKEND_ERROR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[.+\].+backend_error\((\d+\-\d+)\).*").unwrap());

impl FeContext {
    pub fn sync_with_be(&self) -> Result<(), Error> {
        let be2fe_at = join3(&self.working_dir, ".neukgu", "be2fe.json")?;
        let be2fe: Be2Fe = load_json(&be2fe_at)?;
        let fe2be_at = join3(&self.working_dir, ".neukgu", "fe2be.json")?;
        let mut fe2be: Fe2Be = load_json(&fe2be_at)?;
        fe2be.updated_at = Local::now().timestamp();

        for (id, message) in be2fe.to_fe.iter() {
            fe2be.from_be.insert(*id, message.clone());
        }

        for id in be2fe.from_fe.keys() {
            fe2be.to_be.remove(id);
        }

        write_string(
            &fe2be_at,
            &serde_json::to_string_pretty(&fe2be)?,
            FsWriteMode::Atomic,
        )?;
        Ok(())
    }

    pub fn load(working_dir: &str) -> Result<FeContext, Error> {
        let mut context = FeContext::load_without_preview(working_dir)?;
        context.fetch_previews()?;
        Ok(context)
    }

    fn load_without_preview(working_dir: &str) -> Result<FeContext, Error> {
        let log_dir = join3(working_dir, ".neukgu", "logs")?;
        let be_context: ContextJson = load_json(&join3(working_dir, ".neukgu", "context.json")?)?;
        let log_lines = load_logs_tail(&log_dir)?;

        let history: Vec<TurnSummary> = be_context.history.iter().map(
            |t| t.get_turn_summary()
        ).collect();
        let mut curr_tool_call = None;
        let mut truncated_context = None;
        let mut last_api_error = None;
        let mut last_backend_error = None;
        let mut in_turn = true;
        let mut in_session = true;

        for log_line in log_lines.iter().rev() {
            // This is the end of a turn.
            if TOOL_CALL_END_RE.is_match(log_line) {
                in_turn = false;
            }

            else if INIT_LOGGER_RE.is_match(log_line) {
                in_session = false;
            }

            else if in_turn && curr_tool_call.is_none() && let Some(cap) = TOOL_CALL_START_RE.captures(log_line) {
                let log_id = LogId(cap.get(1).unwrap().as_str().to_string());
                let tool_call: ToolCall = serde_json::from_str(&load_log(&log_id, &log_dir)?)?;
                curr_tool_call = Some(tool_call);
            }

            else if truncated_context.is_none() && let Some(cap) = TRUNCATED_CONTEXT_RE.captures(log_line) {
                let log_id = LogId(cap.get(1).unwrap().as_str().to_string());
                let c: Vec<ChosenTurn> = serde_json::from_str(&load_log(&log_id, &log_dir)?)?;
                truncated_context = Some(c);
            }

            else if in_session && last_backend_error.is_none() && let Some(cap) = BACKEND_ERROR_RE.captures(log_line) {
                let log_id = LogId(cap.get(1).unwrap().as_str().to_string());
                let e = load_log(&log_id, &log_dir)?;
                last_backend_error = Some(e);
            }

            else if in_session && last_api_error.is_none() && let Some(cap) = GOT_RESPONSE_RE.captures(log_line) {
                let status = cap.get(1).unwrap().as_str().parse::<u16>().unwrap();

                if status < 200 || status >= 300 {
                    last_api_error = Some(status);
                }
            }
        }

        let truncation = match truncated_context {
            Some(truncated_context) => {
                let mut truncation: HashMap<TurnId, Truncation> = truncated_context.into_iter().map(
                    |t| (t.turn, if t.full_render { Truncation::FullRender } else { Truncation::ShortRender })
                ).collect();

                for TurnSummary { id, .. } in history.iter() {
                    if !truncation.contains_key(id) {
                        truncation.insert(id.clone(), Truncation::Hidden);
                    }
                }

                truncation
            },
            None => history.iter().map(
                |TurnSummary { id, .. }| (id.clone(), Truncation::FullRender)
            ).collect(),
        };

        Ok(FeContext {
            working_dir: working_dir.to_string(),
            history,
            curr_tool_call,
            last_api_error,
            last_backend_error,
            config: Config::load(working_dir)?,
            initialized_at: Instant::now(),
            truncation,
            previews: HashMap::new(),
        })
    }

    fn fetch_previews(&mut self) -> Result<(), Error> {
        let mut new_previews = HashMap::new();

        for turn in self.history.iter() {
            if !self.previews.contains_key(&turn.id) {
                let turn = Turn::load(&turn.id, &self.working_dir)?;
                new_previews.insert(
                    turn.id.clone(),
                    turn.preview(),
                );
            }
        }

        self.previews.extend(new_previews.drain());
        Ok(())
    }

    fn update(&mut self) -> Result<(), Error> {
        *self = FeContext {
            previews: self.previews.clone(),
            initialized_at: self.initialized_at.clone(),
            ..FeContext::load_without_preview(&self.working_dir)?
        };
        self.fetch_previews()?;
        Ok(())
    }

    pub fn start_frame(&mut self) -> Result<(), Error> {
        self.update()?;
        Ok(())
    }

    // user_interrupt: The user asked a question to LLM and the LLM has to answer this.
    // user_response: LLM asked a question to the user and the user answered this.
    pub fn end_frame(
        &mut self,
        pause: Option<bool>,
        user_interrupt: Option<(u64, String)>,
        user_response: Option<(u64, UserResponse)>,
    ) -> Result<(), Error> {
        let fe2be_at = join3(&self.working_dir, ".neukgu", "fe2be.json")?;
        let mut fe2be: Fe2Be = load_json(&fe2be_at)?;

        if let Some(pause) = pause {
            fe2be.pause = pause;
        }

        if let Some((id, interrupt)) = user_interrupt {
            fe2be.to_be.insert(
                id,
                Message {
                    id,
                    kind: MessageKind::User2LLM {
                        question: interrupt,
                        completed: false,
                    },
                },
            );
        }

        if let Some((id, response)) = user_response {
            let Some(Message { id: _, kind: MessageKind::LLM2User { question, answer: None } }) = fe2be.from_be.remove(&id) else { unreachable!() };
            fe2be.to_be.insert(
                id,
                Message {
                    id,
                    kind: MessageKind::LLM2User {
                        question,
                        answer: Some(response),
                    }
                },
            );
        }

        write_string(
            &fe2be_at,
            &serde_json::to_string_pretty(&fe2be)?,
            FsWriteMode::Atomic,
        )?;
        self.sync_with_be()?;
        Ok(())
    }

    pub fn iter_previews(&self) -> Vec<TurnPreview> {
        self.history.iter().map(
            |turn| self.previews.get(&turn.id).unwrap().clone()
        ).collect()
    }

    pub fn is_paused(&self) -> Result<bool, Error> {
        Ok(load_json::<Be2Fe>(&join3(&self.working_dir, ".neukgu", "be2fe.json")?)?.pause)
    }

    pub fn top_bar(&self) -> Result<String, Error> {
        let token_usage: TokenUsage = load_json(&join4(&self.working_dir, ".neukgu", "logs", "tokens.json")?)?;
        let (total_input, total_output) = token_usage.total();
        let (recent_input, recent_output) = token_usage.recent();

        Ok(format!(
            "llm context: {} / {}, neukgu: {}\ntotal input tokens: {}, total output tokens: {}, last 6hrs input tokens: {}, last 6hrs output tokens: {}",
            prettify_bytes(self.get_total_llm_bytes()),
            prettify_bytes(self.config.llm_context_max_len),
            if self.is_paused().unwrap_or(false) || self.is_marked_done().unwrap_or(false) {
                "sleeping"
            } else if self.is_be_busy().unwrap_or(false) {
                "healthy"
            } else if self.is_waking_up() {
                "waking up"
            } else {
                "not responding"
            },
            prettify_tokens(total_input),
            prettify_tokens(total_output),
            prettify_tokens(recent_input),
            prettify_tokens(recent_output),
        ))
    }

    // Push `curr_status` and `curr_error` at the end of turns.
    pub fn curr_status(&self) -> String {
        if self.is_paused().unwrap_or(false) {
            format!("Paused")
        } else if self.is_marked_done().unwrap_or(false) {
            format!("Neukgu has completed his job and is proud of his work!")
        } else if self.get_llm_request().unwrap_or(None).is_some() {
            format!("Neukgu is waiting for the user to answer his question.")
        } else if self.is_be_busy().unwrap_or(false) {
            if let Some(curr_tool_call) = &self.curr_tool_call {
                format!("{} (processing)", curr_tool_call.preview())
            } else {
                format!("Neukgu is thinking...")
            }
        } else {
            if self.is_waking_up() {
                format!("Waking up neukgu...")
            } else {
                format!("Neukgu is not responding")
            }
        }
    }

    pub fn curr_error(&self) -> Option<String> {
        if let Some(code) = self.last_api_error {
            Some(format!("LLM API is not responding... (code {code})"))
        } else if let Some(error) = &self.last_backend_error {
            Some(format!("Internal error in Neukgu: {error}"))
        } else {
            None
        }
    }

    pub fn calc_diff(&self, turn: &Turn) -> Result<Option<String>, Error> {
        if let TurnResult::ToolCallSuccess(ToolCallSuccess::Write { path, content, mode: ToolWriteMode::Truncate, .. }) = &turn.turn_result {
            let file_content_map: FileContent = load_json(&join4(&self.working_dir, ".neukgu", "logs", "files.json")?)?;

            match file_content_map.0.get(path) {
                Some(turn_ids) => {
                    let mut prev_content = String::new();

                    for (i, turn_id) in turn_ids.iter().enumerate() {
                        if turn_id == &turn.id {
                            if i > 0 {
                                let prev_turn = Turn::load(&turn_ids[i - 1], &self.working_dir)?;

                                match &prev_turn.turn_result {
                                    TurnResult::ToolCallSuccess(ToolCallSuccess::ReadText { content, .. } | ToolCallSuccess::Write { content, .. }) => {
                                        prev_content = content.to_string();
                                    },
                                    _ => {
                                        // It's kinda internal error.
                                        return Err(Error::CannotCalcDiff { path: path.to_string(), turn_id: turn.id.clone() });
                                    },
                                }
                            }

                            break;
                        }
                    }

                    return Ok(Some(unified_diff(
                        DiffAlgorithm::Patience,
                        &prev_content,
                        content,
                        5,
                        None,
                    )));
                },
                None => {
                    // There's a bug in backend.
                    return Err(Error::CannotCalcDiff { path: path.to_string(), turn_id: turn.id.clone() });
                },
            }
        }

        Ok(None)
    }

    fn get_total_llm_bytes(&self) -> u64 {
        let mut s = 0;

        for turn in self.history.iter() {
            match self.truncation.get(&turn.id).unwrap() {
                Truncation::Hidden => {},
                Truncation::FullRender => {
                    s += turn.llm_len_full;
                },
                Truncation::ShortRender => {
                    s += turn.llm_len_short;
                },
            }
        }

        s
    }

    // It only checks the write-lock of the be.
    // So, if it returns true, the be is definitely alive.
    // If it returns false, the be is dead or sleeping.
    fn is_be_busy(&self) -> Result<bool, Error> {
        let lock_file_at = join3(&self.working_dir, ".neukgu", ".lock")?;
        let lock_file = std::fs::File::create(&lock_file_at).map_err(|e| FileError::from_std(e, &lock_file_at))?;
        Ok(matches!(lock_file.try_lock(), Err(TryLockError::WouldBlock)))
    }

    // It takes about 0.5~1 second for backend to start.
    // I don't want to print "Neukgu is not responding" at that moment...
    fn is_waking_up(&self) -> bool {
        Instant::now().duration_since(self.initialized_at.clone()).as_millis() < 3000
    }

    pub fn get_llm_request(&self) -> Result<Option<(u64, String)>, Error> {
        let fe2be: Fe2Be = load_json(&join3(&self.working_dir, ".neukgu", "fe2be.json")?)?;

        for Message { id, kind } in fe2be.from_be.values() {
            if let MessageKind::LLM2User { question, answer: None } = kind {
                return Ok(Some((*id, question.to_string())));
            }
        }

        Ok(None)
    }

    pub fn is_marked_done(&self) -> Result<bool, Error> {
        Ok(exists(&join3(&self.working_dir, "logs", "done")?))
    }
}

impl Context {
    pub fn sync_with_fe(&self) -> Result<(), Error> {
        if !self.is_fe_alive()? {
            return Ok(());
        }

        let be2fe_at = join3(&self.working_dir, ".neukgu", "be2fe.json")?;
        let mut be2fe: Be2Fe = load_json(&be2fe_at)?;
        let fe2be_at = join3(&self.working_dir, ".neukgu", "fe2be.json")?;
        let fe2be: Fe2Be = load_json(&fe2be_at)?;
        be2fe.pause = fe2be.pause;

        for (id, message) in fe2be.to_be.iter() {
            be2fe.from_fe.insert(*id, message.clone());
        }

        for (id, old_message) in fe2be.from_be.iter() {
            match be2fe.to_fe.remove(id) {
                // Backend can create response (timeout), so we have to wait until the frontend reads the backend's response.
                Some(new_message) => match (&old_message.kind, &new_message.kind) {
                    (MessageKind::LLM2User { answer: None, .. }, MessageKind::LLM2User { answer: Some(_), .. }) => {
                        be2fe.to_fe.insert(*id, new_message);
                    },
                    _ => {},
                },
                _ => {},
            }
        }

        write_string(
            &be2fe_at,
            &serde_json::to_string_pretty(&be2fe)?,
            FsWriteMode::Atomic,
        )?;
        Ok(())
    }

    pub fn wait_for_fe(&self) -> Result<(), Error> {
        let fe2be_at = join3(&self.working_dir, ".neukgu", "fe2be.json")?;
        let mut is_fe_alive = false;

        for _ in 0..5 {
            let fe2be: Fe2Be = load_json(&fe2be_at)?;
            let curr_timestamp = Local::now().timestamp();

            if fe2be.updated_at + 5 >= curr_timestamp {
                is_fe_alive = true;
                break;
            }

            sleep(Duration::from_millis(2_000));
        }

        if is_fe_alive {
            Ok(())
        }

        else {
            Err(Error::FrontendNotAvailable)
        }
    }

    pub fn is_fe_alive(&self) -> Result<bool, Error> {
        let fe2be_at = join3(&self.working_dir, ".neukgu", "fe2be.json")?;
        let fe2be: Fe2Be = load_json(&fe2be_at)?;
        let curr_timestamp = Local::now().timestamp();
        Ok(fe2be.updated_at + 5 >= curr_timestamp)
    }

    pub fn is_paused(&self) -> Result<bool, Error> {
        Ok(load_json::<Be2Fe>(&join3(&self.working_dir, ".neukgu", "be2fe.json")?)?.pause)
    }

    pub fn ask_to_user(&self, id: u64, question: String) -> Result<(), Error> {
        let be2fe_at = join3(&self.working_dir, ".neukgu", "be2fe.json")?;
        let mut be2fe: Be2Fe = load_json(&be2fe_at)?;
        be2fe.to_fe.insert(
            id,
            Message {
                id,
                kind: MessageKind::LLM2User {
                    question,
                    answer: None,
                },
            },
        );

        write_string(
            &be2fe_at,
            &serde_json::to_string_pretty(&be2fe)?,
            FsWriteMode::Atomic,
        )?;
        Ok(())
    }

    pub fn answer_to_llm(&self, id: u64, question: String, response: UserResponse) -> Result<(), Error> {
        let be2fe_at = join3(&self.working_dir, ".neukgu", "be2fe.json")?;
        let mut be2fe: Be2Fe = load_json(&be2fe_at)?;
        be2fe.to_fe.insert(
            id,
            Message {
                id,
                kind: MessageKind::LLM2User {
                    question,
                    answer: Some(response),
                },
            },
        );

        write_string(
            &be2fe_at,
            &serde_json::to_string_pretty(&be2fe)?,
            FsWriteMode::Atomic,
        )?;
        Ok(())
    }

    pub fn check_user_interrupt(&self) -> Result<Option<(u64, String)>, Error> {
        if !self.is_fe_alive()? {
            return Ok(None);
        }

        let be2fe: Be2Fe = load_json(&join3(&self.working_dir, ".neukgu", "be2fe.json")?)?;

        for message in be2fe.from_fe.values() {
            if let Message { id, kind: MessageKind::User2LLM { question, .. }} = message {
                if !self.completed_user_interrupts.contains(id) {
                    return Ok(Some((*id, question.to_string())));
                }
            }
        }

        Ok(None)
    }

    pub fn check_user_response(&self, id_: u64) -> Result<Option<UserResponse>, Error> {
        let be2fe: Be2Fe = load_json(&join3(&self.working_dir, ".neukgu", "be2fe.json")?)?;

        for message in be2fe.from_fe.values() {
            if let Message { id, kind: MessageKind::LLM2User { answer: Some(answer), ..} } = message && *id == id_ {
                return Ok(Some(answer.clone()));
            }
        }

        Ok(None)
    }
}

fn spawn_backend_process(working_dir: &str) -> Result<(), Error> {
    let bin_path = into_abs_path(&std::env::args().next().unwrap())?;
    Command::new(&bin_path)
        .args(["headless", "--attach-fe"])
        .current_dir(working_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    Ok(())
}
