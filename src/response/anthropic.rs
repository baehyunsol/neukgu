use super::Response;
use crate::Error;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Deserialize, Serialize)]
pub struct AnthropicResponse {
    pub id: String,
    pub role: String,
    pub content: Vec<Map<String, Value>>,
    pub model: String,
    pub stop_reason: String,
    pub usage: AnthropicUsage,
}

#[derive(Deserialize, Serialize)]
pub struct AnthropicUsage {
    pub cache_read_input_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl Response {
    pub fn from_anthropic(s: &str) -> Result<Response, Error> {
        let raw_response: AnthropicResponse = serde_json::from_str(s)?;
        let mut response = String::new();
        let mut thinking = None;

        for content in raw_response.content.iter() {
            match content.get("type") {
                Some(Value::String(ty)) => match ty.as_str() {
                    "text" => match content.get("text") {
                        Some(Value::String(s)) => {
                            response = format!("{response}{s}");
                        },
                        _ => {
                            return Err(Error::FailedToParseAPIResponse(s.to_string()));
                        },
                    },
                    "thinking" => match content.get("thinking") {
                        Some(Value::String(s)) => {
                            assert!(thinking.is_none());
                            thinking = Some(s.to_string());
                        },
                        _ => {
                            return Err(Error::FailedToParseAPIResponse(s.to_string()));
                        },
                    },
                    // We'll just ignore the rest (likely be intermediate results of web_search tool)
                    _ => {},
                },
                _ => {
                    return Err(Error::FailedToParseAPIResponse(s.to_string()));
                },
            }
        }

        Ok(Response {
            response,
            thinking,
            web_search_results: vec![],  // TODO: collect these
            cached_input_tokens: raw_response.usage.cache_read_input_tokens,
            input_tokens: raw_response.usage.input_tokens,
            output_tokens: raw_response.usage.output_tokens,
        })
    }
}
