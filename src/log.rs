use chrono::{Local, SecondsFormat};
use crate::{ChosenTurn, Error, ToolCall, ToolCallError, ToolCallSuccess};
use ragit_fs::{WriteMode, join, write_string};
use serde_json::Value;
use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;

pub struct Logger {
    pub path: String,
}

pub enum LogEntry {
    InitLogger,
    TruncatedContext(Vec<ChosenTurn>),
    SendRequest(reqwest::RequestBuilder),
    RequestBody(Value),
    ReqwestError(String),
    GotResponse(u16),
    ResponseHeader(HashMap<String, String>),
    ResponseText(String),
    TooManyRequests,
    LLMServerBusy,
    ToolCallStart(ToolCall),
    ToolCallEnd(Result<ToolCallSuccess, ToolCallError>),
    AskQuestionToWebBegin(String),
    AskQuestionToWebEnd,
    BackendError(String),
}

impl Logger {
    pub fn new(path: String) -> Self {
        let mut result = Logger { path };
        result.log(LogEntry::InitLogger).unwrap();
        result
    }

    // Many functions rely on the fact that each line in the log file is short.
    // Make sure that each line is shorter than 80 bytes.
    pub fn log(&mut self, entry: LogEntry) -> Result<(), Error> {
        let now = Local::now();
        let extra_content_id = format!("{:07}-{:07}", now.timestamp_millis().max(0) as u64 % 1_000_000_000 / 100, rand::random::<u32>() % 10_000_000);
        let (title, extra_content, extension) = match entry {
            LogEntry::InitLogger => (String::from("init_logger"), None, ""),
            LogEntry::TruncatedContext(c) => (format!("truncated_context({extra_content_id})"), Some(serde_json::to_string_pretty(&c)?), "json"),
            LogEntry::SendRequest(r) => (format!("send_request({extra_content_id})"), Some(format!("{r:?}")), "rs"),
            LogEntry::RequestBody(b) => (format!("request_body({extra_content_id})"), Some(serde_json::to_string_pretty(&b)?), "json"),
            LogEntry::ReqwestError(e) => (format!("reqwest_error({extra_content_id})"), Some(e), "rs"),
            LogEntry::GotResponse(c) => (format!("got_response({c})"), None, ""),
            LogEntry::ResponseHeader(h) => (format!("response_header({extra_content_id})"), Some(serde_json::to_string_pretty(&h)?), "json"),
            LogEntry::ResponseText(t) => (format!("response_text({extra_content_id})"), Some(serde_json::to_string_pretty(&serde_json::from_str::<Value>(&t)?)?), "json"),
            LogEntry::TooManyRequests => (String::from("too_many_requests"), None, ""),
            LogEntry::LLMServerBusy => (String::from("llm_server_busy"), None, ""),
            LogEntry::ToolCallStart(c) => (format!("tool_call_start({extra_content_id})"), Some(serde_json::to_string_pretty(&c)?), "json"),
            LogEntry::ToolCallEnd(c) => (format!("tool_call_end({extra_content_id})"), Some(serde_json::to_string_pretty(&c)?), "json"),
            LogEntry::AskQuestionToWebBegin(q) => (format!("ask_question_to_web_begin({extra_content_id})"), Some(q), "txt"),
            LogEntry::AskQuestionToWebEnd => (format!("ask_question_to_web_end"), None, ""),
            LogEntry::BackendError(e) => (format!("backend_error({extra_content_id})"), Some(e), "rs"),
        };

        write_string(
            &join(&self.path, "log")?,
            &format!(
                "[{}] {title}\n",
                now.to_rfc3339_opts(SecondsFormat::Millis, false),
            ),
            WriteMode::AlwaysAppend,
        )?;

        if let Some(extra_content) = extra_content {
            write_string(
                &join(&self.path, &format!("{extra_content_id}.{extension}"))?,
                &extra_content,
                WriteMode::AlwaysCreate,
            )?;

            // It makes extra_content files are almost always ordered by creation time.
            sleep(Duration::from_millis(100));
        }

        Ok(())
    }
}
