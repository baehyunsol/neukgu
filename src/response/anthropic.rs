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
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl Response {
    pub fn from_anthropic(s: &str) -> Result<Response, Error> {
        let raw_response: AnthropicResponse = serde_json::from_str(s)?;
        let mut response = String::new();
        let mut thinking = None;
        let mut web_search_results = vec![];  // TODO: collect this

        for content in raw_response.content.iter() {
            match content.get("type") {
                Some(Value::String(s)) => match s.as_str() {
                    "text" => match content.get("text") {
                        Some(Value::String(s)) => {
                            response = format!("{response}{s}");
                        },
                        _ => unreachable!(),
                    },
                    "thinking" => match content.get("thinking") {
                        Some(Value::String(s)) => {
                            assert!(thinking.is_none());
                            thinking = Some(s.to_string());
                        },
                        _ => unreachable!(),
                    },
                    // We'll just ignore the rest (likely be intermediate results of web_search tool)
                    _ => {},
                },
                _ => unreachable!(),
            }
        }

        Ok(Response {
            response,
            thinking,
            web_search_results,
            input_tokens: raw_response.usage.input_tokens,
            output_tokens: raw_response.usage.output_tokens,
        })
    }
}
