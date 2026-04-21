use chrono::{Local, SecondsFormat};
use crate::{
    ChosenTurn,
    Error,
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
        LogId(format!("{:07}-{:07}", now.timestamp_millis().max(0) as u64 % 1_000_000_000 / 100, rand::random::<u32>() % 10_000_000))
    }
}

pub struct Logger {
    // `<working_dir>/.neukgu/logs/`
    pub log_dir: String,
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
    pub fn new(log_dir: &str) -> Self {
        let result = Logger { log_dir: log_dir.to_string() };
        result.log(LogEntry::InitLogger).unwrap();
        result
    }

    // Many functions rely on the fact that each line in the log file is short.
    // Make sure that each line is shorter than 80 bytes.
    pub fn log(&self, entry: LogEntry) -> Result<(), Error> {
        let now = Local::now();
        let log_id = LogId::new();
        let (title, extra_content) = match entry {
            LogEntry::InitLogger => (String::from("init_logger"), None),
            LogEntry::TruncatedContext(c) => (format!("truncated_context({})", log_id.0), Some(serde_json::to_string_pretty(&c)?)),
            LogEntry::SendRequest(r) => (format!("send_request({})", log_id.0), Some(format!("{r:?}"))),
            LogEntry::RequestBody(b) => (format!("request_body({})", log_id.0), Some(serde_json::to_string_pretty(&b)?)),
            LogEntry::ReqwestError(e) => (format!("reqwest_error({})", log_id.0), Some(e)),
            LogEntry::GotResponse(c) => (format!("got_response({c})"), None),
            LogEntry::ResponseHeader(h) => (format!("response_header({})", log_id.0), Some(serde_json::to_string_pretty(&h)?)),
            LogEntry::ResponseText(t) => (format!("response_text({})", log_id.0), Some(serde_json::to_string_pretty(&serde_json::from_str::<Value>(&t)?)?)),
            LogEntry::TooManyRequests => (String::from("too_many_requests"), None),
            LogEntry::LLMServerBusy => (String::from("llm_server_busy"), None),
            LogEntry::ToolCallStart(c) => (format!("tool_call_start({})", log_id.0), Some(serde_json::to_string_pretty(&c)?)),
            LogEntry::ToolCallEnd(c) => (format!("tool_call_end({})", log_id.0), Some(serde_json::to_string_pretty(&c)?)),
            LogEntry::AskQuestionToWebBegin(q) => (format!("ask_question_to_web_begin({})", log_id.0), Some(q)),
            LogEntry::AskQuestionToWebEnd => (format!("ask_question_to_web_end"), None),
            LogEntry::BackendError(e) => (format!("backend_error({})", log_id.0), Some(e)),
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
            let mut compressed = vec![];
            let mut gz = GzEncoder::new(extra_content.as_bytes(), Compression::new(2));
            gz.read_to_end(&mut compressed)?;

            write_bytes(
                &join(&self.log_dir, &log_id.0)?,
                &compressed,
                WriteMode::AlwaysCreate,
            )?;

            // It makes extra_content files are almost always ordered by creation time.
            sleep(Duration::from_millis(100));
        }

        Ok(())
    }

    pub fn log_api_usage(&self, input: u64, output: u64) -> Result<(), Error> {
        let usage_at = join(&self.log_dir, "tokens.json")?;
        let now = Local::now().timestamp().max(0) as u64 / 3600;
        let mut usage: TokenUsage = load_json(&usage_at)?;

        match usage.0.entry(now) {
            Entry::Occupied(mut e) => {
                let e = e.get_mut();
                e.0 += input;
                e.1 += output;
            },
            Entry::Vacant(e) => {
                e.insert((input, output));
            },
        }

        write_string(
            &usage_at,
            &serde_json::to_string_pretty(&usage)?,
            WriteMode::Atomic,
        )?;
        Ok(())
    }

    // It logs every text files that neukgu reads/writes.
    // It's later used to
    //    1. make sure that no files are modified while neukgu is sleeping
    //    2. calc diff of writes
    pub fn log_file_content(&self, path: &str, turn_id: &TurnId) -> Result<(), Error> {
        let map_at = join(&self.log_dir, "files.json")?;
        let mut map: FileContent = load_json(&map_at)?;

        // `path` is always normalized, so we can use it as a key
        match map.0.entry(path.to_string()) {
            Entry::Occupied(mut e) => {
                e.get_mut().push(turn_id.clone());
            },
            Entry::Vacant(e) => {
                e.insert(vec![turn_id.clone()]);
            },
        }

        write_string(
            &map_at,
            &serde_json::to_string_pretty(&map)?,
            WriteMode::Atomic,
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TokenUsage(HashMap<u64  /* timestamp */, (u64 /* input */, u64 /* output */)>);

impl TokenUsage {
    pub fn total(&self) -> (u64, u64) {
        self.0.values().fold((0, 0), |(i, o), (ii, oo)| (i + *ii, o + *oo))
    }

    pub fn recent(&self) -> (u64, u64) {
        let now = Local::now().timestamp().max(0) as u64 / 3600;
        let recent_6 = (now.max(5) - 5)..(now + 1);
        recent_6.map(
            |k| self.0.get(&k).cloned().unwrap_or((0, 0))
        ).fold((0, 0), |(i, o), (ii, oo)| (i + ii, o + oo))
    }
}

// It only remembers the log_id of the read/write operation.
// In order to get the actual content, you have to read the turn file.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileContent(pub HashMap<String, Vec<TurnId>>);

pub fn load_log(id: &LogId, log_dir: &str) -> Result<String, Error> {
    let path = join(log_dir, &id.0)?;
    let bytes = read_bytes(&path)?;
    let mut decompressed = vec![];
    let mut gz = GzDecoder::new(&bytes[..]);
    gz.read_to_end(&mut decompressed)?;
    Ok(String::from_utf8(decompressed)?)
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
