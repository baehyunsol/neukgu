use chrono::Local;
use crate::{
    ChosenTurn,
    Config,
    Context,
    ContextJson,
    Error,
    Interrupt,
    ToolCall,
    Turn,
    TurnId,
    TurnPreview,
    TurnSummary,
    load_json,
    load_log_tail,
    prettify_bytes,
};
use ragit_fs::{
    FileError,
    WriteMode,
    exists,
    join,
    join3,
    read_string,
    write_string,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::TryLockError;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::thread::sleep;
use std::time::Duration;

pub mod tui;
pub mod gui;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Be2Fe {
    pub completed_user_request: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Fe2Be {
    pub pause: bool,
    pub user_request: Option<(u64, String)>,
    pub updated_at: i64,
}

impl Default for Fe2Be {
    fn default() -> Fe2Be {
        Fe2Be {
            pause: false,
            user_request: None,
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
    pub history: Vec<TurnSummary>,
    pub curr_tool_call: Option<ToolCall>,
    pub config: Config,

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
    pub fn load() -> Result<FeContext, Error> {
        let mut context = FeContext::load_without_preview()?;
        context.fetch_previews()?;
        Ok(context)
    }

    fn load_without_preview() -> Result<FeContext, Error> {
        if !exists(&join(".neukgu", "fe2be.json")?) {
            write_string(
                &join(".neukgu", "fe2be.json")?,
                &serde_json::to_string(&Fe2Be::default())?,
                WriteMode::Atomic,
            )?;
        }

        let be_context: ContextJson = load_json(&join(".neukgu", "context.json")?)?;
        let log_lines = load_log_tail()?;

        let history: Vec<TurnSummary> = be_context.history.iter().map(
            |t| t.get_turn_summary()
        ).collect();
        let mut curr_tool_call = None;
        let mut truncated_context = None;
        let mut last_api_error = None;
        let mut last_backend_error = None;

        for log_line in log_lines.iter().rev() {
            // This is the end of a turn.
            if TOOL_CALL_END_RE.is_match(log_line) || INIT_LOGGER_RE.is_match(log_line) {
                break;
            }

            else if let Some(cap) = TOOL_CALL_START_RE.captures(log_line) {
                let log_id = cap.get(1).unwrap().as_str().to_string();
                let tool_call: ToolCall = load_json(&join3(".neukgu", "logs", &format!("{log_id}.json"))?)?;
                curr_tool_call = Some(tool_call);
            }

            else if let Some(cap) = TRUNCATED_CONTEXT_RE.captures(log_line) {
                let log_id = cap.get(1).unwrap().as_str().to_string();
                let c: Vec<ChosenTurn> = load_json(&join3(".neukgu", "logs", &format!("{log_id}.json"))?)?;
                truncated_context = Some(c);
            }

            else if let Some(cap) = BACKEND_ERROR_RE.captures(log_line) {
                let log_id = cap.get(1).unwrap().as_str().to_string();
                let e = read_string(&join3(".neukgu", "logs", &format!("{log_id}.rs"))?)?;
                last_backend_error = Some(e);
            }

            else if let Some(cap) = GOT_RESPONSE_RE.captures(log_line) {
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
            history,
            curr_tool_call,
            last_api_error,
            last_backend_error,
            config: Config::load()?,
            truncation,
            previews: HashMap::new(),
        })
    }

    fn fetch_previews(&mut self) -> Result<(), Error> {
        let mut new_previews = HashMap::new();

        for turn in self.history.iter() {
            if !self.previews.contains_key(&turn.id) {
                let turn = Turn::load(&turn.id)?;
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
            ..FeContext::load_without_preview()?
        };
        self.fetch_previews()?;
        Ok(())
    }

    pub fn start_frame(&mut self) -> Result<(), Error> {
        self.update()?;
        Ok(())
    }

    // Interrupt::Request: The user asked a question to LLM and the LLM has to answer this.
    // user_response: LLM asked a question to the user and the user answered this.
    pub fn end_frame(&mut self, interrupt: Option<Interrupt>, user_response: Option<(u64, String)>) -> Result<(), Error> {
        match interrupt {
            Some(Interrupt::Pause) => {
                self.pause()?;
            },
            Some(Interrupt::Resume) => {
                self.resume()?;
            },
            Some(Interrupt::Request { .. }) => todo!(),
            None => {},
        }

        if let Some(user_response) = user_response {
            todo!()
        }

        let fe2be_at = join(".neukgu", "fe2be.json")?;
        let mut fe2be: Fe2Be = load_json(&fe2be_at)?;
        fe2be.updated_at = Local::now().timestamp();
        write_string(
            &fe2be_at,
            &serde_json::to_string(&fe2be)?,
            WriteMode::Atomic,
        )?;

        Ok(())
    }

    pub fn iter_previews(&self) -> Vec<TurnPreview> {
        self.history.iter().map(
            |turn| self.previews.get(&turn.id).unwrap().clone()
        ).collect()
    }

    fn pause(&mut self) -> Result<(), Error> {
        let fe2be_at = join(".neukgu", "fe2be.json")?;
        let mut fe2be: Fe2Be = load_json(&fe2be_at)?;
        fe2be.pause = true;
        write_string(
            &fe2be_at,
            &serde_json::to_string(&fe2be)?,
            WriteMode::Atomic,
        )?;
        Ok(())
    }

    fn resume(&mut self) -> Result<(), Error> {
        let fe2be_at = join(".neukgu", "fe2be.json")?;
        let mut fe2be: Fe2Be = load_json(&fe2be_at)?;
        fe2be.pause = false;
        write_string(
            &fe2be_at,
            &serde_json::to_string(&fe2be)?,
            WriteMode::Atomic,
        )?;
        Ok(())
    }

    pub fn is_paused(&self) -> bool {
        match load_json::<Fe2Be>(&join(".neukgu", "fe2be.json").unwrap()) {
            Ok(f) => f.pause,
            Err(_) => false,
        }
    }

    pub fn top_bar(&self) -> String {
        format!(
            "llm context: {}, neukgu: {}",
            prettify_bytes(self.get_total_llm_bytes()),
            if self.is_paused() {
                "paused"
            } else if self.is_be_busy().unwrap_or(false) {
                "healthy"
            } else {
                "not responding"
            },
        )
    }

    // Push `curr_status` and `curr_error` at the end of turns.
    pub fn curr_status(&self) -> String {
        if self.is_paused() {
            format!("Paused")
        } else if !self.is_be_busy().unwrap_or(false) {
            format!("Neukgu is not responding")
        } else if let Some(curr_tool_call) = &self.curr_tool_call {
            format!("{} (processing)", curr_tool_call.preview())
        } else {
            format!("Neukgu is thinking...")
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
        let lock_file = std::fs::File::create(".neukgu/.lock").map_err(|e| FileError::from_std(e, ".neukgu/.lock"))?;
        Ok(matches!(lock_file.try_lock(), Err(TryLockError::WouldBlock)))
    }
}

impl Context {
    pub fn wait_for_fe(&self) -> Result<(), Error> {
        let fe2be_at = join(".neukgu", "fe2be.json")?;
        let mut is_fe_alive = false;

        // The frontend will update `fe2be.json` at least once per 5 seconds.
        for _ in 0..5 {
            if !exists(&fe2be_at) {
                sleep(Duration::from_millis(3_000));
                continue;
            }
        }

        for _ in 0..5 {
            let fe2be: Fe2Be = load_json(&fe2be_at)?;
            let curr_timestamp = Local::now().timestamp();

            if fe2be.updated_at + 10 >= curr_timestamp {
                is_fe_alive = true;
                break;
            }

            sleep(Duration::from_millis(3_000));
        }

        if is_fe_alive {
            Ok(())
        }

        else {
            Err(Error::FrontendNotAvailable)
        }
    }

    pub fn is_fe_alive(&self) -> Result<bool, Error> {
        let fe2be_at = join(".neukgu", "fe2be.json")?;

        if exists(&fe2be_at) {
            let fe2be: Fe2Be = load_json(&fe2be_at)?;
            let curr_timestamp = Local::now().timestamp();
            Ok(fe2be.updated_at + 10 >= curr_timestamp)
        }

        else {
            Ok(false)
        }
    }
}

fn spawn_backend_process() -> Result<(), Error> {
    Command::new(std::env::args().next().unwrap())
        .args(["headless", "--attach-fe"])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    Ok(())
}
