use super::Response;
use crate::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Deserialize, Serialize)]
pub struct OpenAiResponse {
    output: Vec<Value>,
    usage: OpenAiUsage,
}

#[derive(Deserialize, Serialize)]
pub struct OpenAiUsage {
    input_tokens: u64,
    input_tokens_details: InputTokensDetails,
    output_tokens: u64,
}

#[derive(Deserialize, Serialize)]
struct InputTokensDetails {
    cached_tokens: u64,
}

impl Response {
    pub fn from_openai(s: &str) -> Result<Response, Error> {
        let raw_response: OpenAiResponse = serde_json::from_str(s)?;
        let mut response = String::new();
        let mut thinking = None;

        for output in raw_response.output.iter() {
            match output.get("type") {
                Some(Value::String(ty)) => match ty.as_str() {
                    "message" => {  // ["content"]["text"] or ["content"]["refusal"]
                        let Some(Value::Array(messages)) = output.get("content") else {
                            return Err(Error::FailedToParseAPIResponse(s.to_string()));
                        };

                        for message in messages.iter() {
                            let s = match (message.get("text"), message.get("refusal")) {
                                (Some(Value::String(s)), _) | (_, Some(Value::String(s))) => s.to_string(),
                                _ => {
                                    return Err(Error::FailedToParseAPIResponse(s.to_string()));
                                },
                            };

                            response = format!("{response}{s}");
                        }
                    },
                    "web_search_call" => {},  // TODO
                    "reasoning" => {  // ["summary"]["text"] or ["content"]["text"]
                        let message = match (output.get("summary"), output.get("content")) {
                            (_, Some(obj)) | (Some(obj), _) => obj,
                            _ => {
                                return Err(Error::FailedToParseAPIResponse(s.to_string()));
                            },
                        };
                        let Some(Value::String(s)) = message.get("text") else {
                            return Err(Error::FailedToParseAPIResponse(s.to_string()));
                        };
                        thinking = Some(s.to_string());
                    },
                    // We'll just ignore the rest
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
            web_search_results: vec![],
            cached_input_tokens: raw_response.usage.input_tokens_details.cached_tokens,
            input_tokens: raw_response.usage.input_tokens,
            output_tokens: raw_response.usage.output_tokens,
        })
    }
}
