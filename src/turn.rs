use chrono::{Local, SecondsFormat};
use crate::{
    Config,
    Error,
    LLMToken,
    ParseError,
    ParsedSegment,
    ToolCallError,
    ToolCallSuccess,
    count_bytes_of_llm_tokens,
    get_first_tool_call,
};
use flate2::Compression;
use flate2::read::{GzDecoder, GzEncoder};
use ragit_fs::{
    WriteMode,
    join3,
    read_bytes,
    write_bytes,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::sync::LazyLock;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct TurnId(pub String);

pub static TURN_ID_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r".+(pe|tce|tcs)\-(\d+)\-(\d+)$").unwrap());
pub static TURN_TIMESTAMP_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.+)\-(?:pe|tce|tcs)\-.+").unwrap());

impl TurnId {
    pub fn dummy() -> TurnId {
        TurnId(String::new())
    }

    pub fn get_turn_summary(&self) -> TurnSummary {
        let cap = TURN_ID_REGEX.captures(&self.0).unwrap();
        let result = match cap.get(1).unwrap().as_str() {
            "pe" => TurnResultSummary::ParseError,
            "tce" => TurnResultSummary::ToolCallError,
            "tcs" => TurnResultSummary::ToolCallSuccess,
            _ => unreachable!(),
        };
        let llm_len_short = cap.get(2).unwrap().as_str().parse::<u64>().unwrap();
        let llm_len_full = cap.get(3).unwrap().as_str().parse::<u64>().unwrap();
        TurnSummary { id: self.clone(), result, llm_len_short, llm_len_full }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Turn {
    pub id: TurnId,
    pub raw_response: String,
    pub parse_result: Option<Vec<ParsedSegment>>,
    pub turn_result: TurnResult,
    pub llm_elapsed_ms: u64,
    pub tool_elapsed_ms: u64,

    // Currently, user-interrupt is implemented by inserting a fake turn
    // with `Ask { to: User }`.
    pub is_fake: bool,
}

impl Turn {
    pub fn new(
        raw_response: String,
        parse_result: Option<Vec<ParsedSegment>>,
        turn_result: TurnResult,
        llm_elapsed_ms: u64,
        tool_elapsed_ms: u64,
        is_fake: bool,
        config: &Config,
    ) -> Turn {
        let mut turn = Turn {
            id: TurnId::dummy(),
            raw_response,
            parse_result,
            turn_result,
            llm_elapsed_ms,
            tool_elapsed_ms,
            is_fake,
        };
        let turn_summary = turn.summary(config);
        let turn_id = get_turn_id(turn_summary);
        turn.id = turn_id;
        turn
    }

    pub fn load(id: &TurnId) -> Result<Turn, Error> {
        let path = join3(".neukgu", "turns", &format!("{}.json", id.0))?;
        let json = read_bytes(&path)?;
        let mut decompressed = vec![];
        let mut gz = GzDecoder::new(&json[..]);
        gz.read_to_end(&mut decompressed)?;
        Ok(serde_json::from_slice(&decompressed)?)
    }

    pub fn store(&self) -> Result<(), Error> {
        let json = serde_json::to_vec(self)?;
        let mut compressed = vec![];
        let mut gz = GzEncoder::new(&json[..], Compression::new(2));
        gz.read_to_end(&mut compressed)?;

        Ok(write_bytes(
            &join3(".neukgu", "turns", &format!("{}.json", self.id.0))?,
            &compressed,
            WriteMode::Atomic,
        )?)
    }

    // `.summary()` is used by be and fe, and `.preview()` is used by fe.
    pub fn summary(&self, config: &Config) -> TurnSummary {
        let result_len = count_bytes_of_llm_tokens(
            &self.turn_result.to_llm_tokens(config),

            // TODO: make it configurable
            /* bytes_per_image: */ 2048,
        );
        let response_len_full = self.raw_response.len() as u64;
        let response_len_short = match &self.parse_result {
            Some(segments) => {
                let Some(ParsedSegment::ToolCall { input, .. }) = get_first_tool_call(segments) else { unreachable!() };
                input.len() as u64
            },
            None => response_len_full,
        };

        TurnSummary {
            id: self.id.clone(),
            result: self.turn_result.summary(),
            llm_len_short: result_len + response_len_short,
            llm_len_full: result_len + response_len_full,
        }
    }

    // `.summary()` is used by be and fe, and `.preview()` is used by fe.
    pub fn preview(&self) -> TurnPreview {
        let preview_title = match &self.parse_result {
            Some(parse_result) => {
                if self.is_user_interrupt() {
                    String::from("User interrupt")
                }

                else if let Some(ParsedSegment::ToolCall { call, .. }) = get_first_tool_call(parse_result) {
                    call.preview()
                }

                else {
                    String::from("???")
                }
            },
            _ => String::from("????"),
        };

        TurnPreview {
            id: self.id.clone(),
            preview_title,
            result: self.turn_result.summary(),
            llm_elapsed_ms: self.llm_elapsed_ms,
            tool_elapsed_ms: self.tool_elapsed_ms,
            timestamp: TURN_TIMESTAMP_REGEX.captures(&self.id.0).unwrap().get(1).unwrap().as_str().to_string(),
        }
    }

    // As of now, this is the only condition to check...
    pub fn is_user_interrupt(&self) -> bool {
        self.is_fake
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum TurnResult {
    ParseError(ParseError),
    ToolCallError(ToolCallError),
    ToolCallSuccess(ToolCallSuccess),
}

impl TurnResult {
    pub fn to_llm_tokens(&self, config: &Config) -> Vec<LLMToken> {
        match self {
            TurnResult::ParseError(e) => e.to_llm_tokens(),
            TurnResult::ToolCallError(e) => e.to_llm_tokens(),
            TurnResult::ToolCallSuccess(r) => r.to_llm_tokens(config),
        }
    }

    pub fn summary(&self) -> TurnResultSummary {
        match self {
            TurnResult::ParseError(_) => TurnResultSummary::ParseError,
            TurnResult::ToolCallError(_) => TurnResultSummary::ToolCallError,
            TurnResult::ToolCallSuccess(_) => TurnResultSummary::ToolCallSuccess,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Serialize, PartialEq)]
pub enum TurnResultSummary {
    ParseError,
    ToolCallError,
    ToolCallSuccess,
}

#[derive(Clone, Debug)]
pub struct TurnSummary {
    pub id: TurnId,
    pub result: TurnResultSummary,
    pub llm_len_short: u64,
    pub llm_len_full: u64,
}

pub fn get_turn_id(summary: TurnSummary) -> TurnId {
    let now = Local::now();
    let now = now.to_rfc3339_opts(SecondsFormat::Millis, true);
    TurnId(format!(
        "{now}-{}-{:06}-{:06}",
        match summary.result {
            TurnResultSummary::ParseError => "pe",
            TurnResultSummary::ToolCallError => "tce",
            TurnResultSummary::ToolCallSuccess => "tcs",
        },
        summary.llm_len_short,
        summary.llm_len_full,
    ))
}

#[derive(Clone, Debug)]
pub struct TurnPreview {
    pub id: TurnId,
    pub preview_title: String,
    pub result: TurnResultSummary,
    pub llm_elapsed_ms: u64,
    pub tool_elapsed_ms: u64,

    // When the tool-call ended
    pub timestamp: String,
}
