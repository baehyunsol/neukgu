use chrono::{Local, SecondsFormat};
use crate::{
    ChosenTurn,
    Error,
    Model,
    SessionId,
    ToolCall,
    ToolCallError,
    ToolCallSuccess,
    TurnId,
    load_json,
};
use flate2::Compression;
use flate2::read::{GzDecoder, GzEncoder};
use ragit_fs::{
    WriteMode,
    exists,
    file_size,
    join,
    read_bytes,
    read_bytes_offset,
    read_string,
    write_bytes,
    write_string,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::hash_map::{Entry, HashMap};
use std::io::Read;
use std::thread::sleep;
use std::time::Duration;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LogId(pub String);

impl LogId {
    pub fn new() -> Self {
        let now = Local::now();
        LogId(format!("{:08}-{:05}", now.timestamp_millis().max(0) as u64 % 2_000_000_000 / 20, rand::random::<u32>() % 100_000))
    }
}

pub struct Logger {
    // `<working_dir>/.neukgu/logs/`
    pub log_dir: String,

    // If set, it logs the token usage globally.
    pub global_log_dir: Option<String>,

    pub compress: bool,
    pub enabled: bool,
}

pub enum LogEntry {
    InitLogger,
    TruncatedContext(Vec<ChosenTurn>),
    SendRequest(reqwest::RequestBuilder),
    RequestBody(Value),
    ReqwestError(String),
    GotResponse(u16),
    GotImageResponse(u16),
    ResponseHeader(HashMap<String, String>),
    ResponseText(String),
    TooManyRequests,
    LLMServerBusy,
    ToolCallStart(ToolCall),
    ToolCallEnd(Result<ToolCallSuccess, ToolCallError>),
    QuestionFromUserStart(String),
    QuestionFromUserEnd,
    WriteFinalReportStart,
    WriteFinalReportEnd,
    AskQuestionToWebBegin(String),
    AskQuestionToWebEnd,
    BackendError(String),
    KillBackend,
    RollBack(TurnId),
    SwitchContext { old: SessionId, new: SessionId },
    UserInterruptWhileLLMRequest,
    UserInterruptWhileToolCall,
}

impl Logger {
    pub fn new(
        log_dir: String,
        global_log_dir: Option<String>,
        compress: bool,
        enabled: bool,
    ) -> Self {
        let result = Logger { log_dir, global_log_dir, compress, enabled };
        result.log(LogEntry::InitLogger).unwrap();
        result
    }

    // Many functions rely on the fact that each line in the log file is short.
    // Make sure that each line is shorter than 80 bytes.
    // If it creates an extra log file, it returns the LogId of the extra log.
    pub fn log(&self, entry: LogEntry) -> Result<Option<LogId>, Error> {
        if !self.enabled { return Ok(None); }

        let now = Local::now();
        let log_id = LogId::new();
        let (title, extra_content, extension) = match entry {
            LogEntry::InitLogger => (String::from("init_logger"), None, ""),
            LogEntry::TruncatedContext(c) => (format!("truncated_context({})", log_id.0), Some(serde_json::to_string_pretty(&c)?), "json"),
            LogEntry::SendRequest(r) => (format!("send_request({})", log_id.0), Some(format!("{r:?}")), "rs"),
            LogEntry::RequestBody(b) => (format!("request_body({})", log_id.0), Some(serde_json::to_string_pretty(&b)?), "json"),
            LogEntry::ReqwestError(e) => (format!("reqwest_error({})", log_id.0), Some(e), "rs"),
            LogEntry::GotResponse(c) => (format!("got_response({c})"), None, ""),
            LogEntry::GotImageResponse(c) => (format!("got_image_response({c})"), None, ""),
            LogEntry::ResponseHeader(h) => (format!("response_header({})", log_id.0), Some(serde_json::to_string_pretty(&h)?), "json"),
            LogEntry::ResponseText(t) => (format!("response_text({})", log_id.0), Some(serde_json::to_string_pretty(&serde_json::from_str::<Value>(&t)?)?), "json"),
            LogEntry::TooManyRequests => (String::from("too_many_requests"), None, ""),
            LogEntry::LLMServerBusy => (String::from("llm_server_busy"), None, ""),
            LogEntry::ToolCallStart(c) => (format!("tool_call_start({})", log_id.0), Some(serde_json::to_string_pretty(&c)?), "json"),
            LogEntry::ToolCallEnd(c) => (format!("tool_call_end({})", log_id.0), Some(serde_json::to_string_pretty(&c)?), "json"),
            LogEntry::QuestionFromUserStart(q) => (format!("question_from_user_start({})", log_id.0), Some(q), "txt"),
            LogEntry::QuestionFromUserEnd => (format!("question_from_user_end"), None, ""),
            LogEntry::WriteFinalReportStart => (format!("write_final_report_start"), None, ""),
            LogEntry::WriteFinalReportEnd => (format!("write_final_report_end"), None, ""),
            LogEntry::AskQuestionToWebBegin(q) => (format!("ask_question_to_web_begin({})", log_id.0), Some(q), "txt"),
            LogEntry::AskQuestionToWebEnd => (format!("ask_question_to_web_end"), None, ""),
            LogEntry::BackendError(e) => (format!("backend_error({})", log_id.0), Some(e), "rs"),
            LogEntry::KillBackend => (format!("kill_backend"), None, ""),
            LogEntry::RollBack(t) => (format!("roll_back({})", log_id.0), Some(t.0), "txt"),
            LogEntry::SwitchContext { old, new } => (format!("switch_context({:04x} -> {:04x})", old.0 >> 48, new.0 >> 48), None, ""),
            LogEntry::UserInterruptWhileLLMRequest => (format!("user_interrupt_while_llm_request"), None, ""),
            LogEntry::UserInterruptWhileToolCall => (format!("user_interrupt_while_tool_call"), None, ""),
        };

        write_string(
            &join(&self.log_dir, "log")?,
            &format!(
                "[{}] {title}\n",
                now.to_rfc3339_opts(SecondsFormat::Millis, false),
            ),
            WriteMode::AlwaysAppend,
        )?;

        if let Some(extra_content) = extra_content {
            if self.compress {
                let mut compressed = vec![];
                let mut gz = GzEncoder::new(extra_content.as_bytes(), Compression::new(3));
                gz.read_to_end(&mut compressed)?;

                write_bytes(
                    &join(&self.log_dir, &format!("{}.{extension}.gz", log_id.0))?,
                    &compressed,
                    WriteMode::AlwaysCreate,
                )?;
            }

            else {
                write_string(
                    &join(&self.log_dir, &format!("{}.{extension}", log_id.0))?,
                    &extra_content,
                    WriteMode::AlwaysCreate,
                )?;
            }

            // It makes extra_content files are almost always ordered by creation time.
            sleep(Duration::from_millis(20));
            Ok(Some(log_id))
        }

        else {
            Ok(None)
        }
    }

    pub fn log_token_usage(&self, model: Model, cached_input: u64, input: u64, output: u64) -> Result<(), Error> {
        if !self.enabled { return Ok(()); }

        let now = Local::now().timestamp().max(0) as u64 / 3600;
        let local_token_usage = Some(join(&self.log_dir, "tokens.json")?);
        let global_token_usage = if let Some(global_log_dir) = &self.global_log_dir {
            Some(join(global_log_dir, "tokens.json")?)
        } else {
            None
        };

        for usage_at in [
            local_token_usage,
            global_token_usage,
        ] {
            let Some(usage_at) = usage_at else { continue };
            let mut usage: TokenUsage = load_json(&usage_at)?;

            match usage.0.entry(now) {
                Entry::Occupied(mut e) => match e.get_mut().entry(model) {
                    Entry::Occupied(mut e) => {
                        let e = e.get_mut();
                        e.0 += cached_input;
                        e.1 += input;
                        e.2 += output;
                    },
                    Entry::Vacant(e) => {
                        e.insert((cached_input, input, output));
                    },
                },
                Entry::Vacant(e) => {
                    e.insert([(model, (cached_input, input, output))].into_iter().collect());
                },
            }

            write_string(
                &usage_at,
                &serde_json::to_string_pretty(&usage)?,
                WriteMode::Atomic,
            )?;
        }

        Ok(())
    }

    pub fn log_image_edit_token_usage(&self) -> Result<(), Error> {
        // TODO
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TokenUsage(HashMap<u64  /* timestamp */, HashMap<Model, (u64 /* cached_input */, u64 /* input */, u64 /* output */)>>);

impl TokenUsage {
    pub fn is_empty(&self) -> bool {
        for usage in self.0.values() {
            for (cached_input, input, output) in usage.values() {
                if cached_input + input + output > 0 {
                    return false;
                }
            }
        }

        true
    }

    pub fn total(&self) -> HashMap<Model, (u64, u64, u64)> {
        let mut result: HashMap<Model, (u64, u64, u64)> = HashMap::new();

        for usage in self.0.values() {
            for (model, (cached_input, input, output)) in usage.iter() {
                match result.entry(*model) {
                    Entry::Occupied(mut e) => {
                        let e = e.get_mut();
                        e.0 += cached_input;
                        e.1 += input;
                        e.2 += output;
                    },
                    Entry::Vacant(e) => {
                        e.insert((*cached_input, *input, *output));
                    },
                }
            }
        }

        result
    }

    pub fn recent(&self) -> HashMap<Model, (u64, u64, u64)> {
        let now = Local::now().timestamp().max(0) as u64 / 3600;
        let recent_6 = (now.max(5) - 5)..(now + 1);
        let mut result: HashMap<Model, (u64, u64, u64)> = HashMap::new();

        for t in recent_6 {
            if let Some(usage) = self.0.get(&t) {
                for (model, (cached_input, input, output)) in usage.iter() {
                    match result.entry(*model) {
                        Entry::Occupied(mut e) => {
                            let e = e.get_mut();
                            e.0 += cached_input;
                            e.1 += input;
                            e.2 += output;
                        },
                        Entry::Vacant(e) => {
                            e.insert((*cached_input, *input, *output));
                        },
                    }
                }
            }
        }

        result
    }
}

pub fn load_log(id: &LogId, log_dir: &str) -> Result<(/* log */ String, /* extension */ String), Error> {
    for extension in ["json", "rs", "txt"] {
        let path1 = join(log_dir, &format!("{}.{extension}.gz", id.0))?;
        let path2 = join(log_dir, &format!("{}.{extension}", id.0))?;

        if exists(&path1) {
            let bytes = read_bytes(&path1)?;
            let mut decompressed = vec![];
            let mut gz = GzDecoder::new(&bytes[..]);
            gz.read_to_end(&mut decompressed)?;
            return Ok((String::from_utf8(decompressed)?, extension.to_string()));
        }

        if exists(&path2) {
            return Ok((read_string(&path2)?, extension.to_string()));
        }
    }

    Err(Error::InvalidLogId(id.clone()))
}

pub fn load_logs_tail(log_dir: &str) -> Result<Vec<String>, Error> {
    let path = join(log_dir, "log")?;
    let file_size = file_size(&path)?;

    if file_size > 8192 {
        let log_tail = String::from_utf8_lossy(&read_bytes_offset(&path, file_size - 8192, file_size)?).to_string();

        // first line is incomplete
        Ok(log_tail.lines().skip(1).map(|s| s.to_string()).collect())
    }

    else {
        Ok(read_string(&path)?.lines().map(|s| s.to_string()).collect())
    }
}
