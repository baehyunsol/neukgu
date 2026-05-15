use super::{Response, WebSearchResult};
use crate::{Error, ApiLog};
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
        let mut web_search_results = vec![];

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

                            if let Some(Value::Array(annotations)) = message.get("annotations") {
                                for annotation in annotations.iter() {
                                    if let Some(Value::String(r#type)) = annotation.get("type") && r#type == "url_citation" {
                                        let Some(Value::String(title)) = annotation.get("title") else {
                                            return Err(Error::FailedToParseAPIResponse(s.to_string()));
                                        };
                                        let Some(Value::String(url)) = annotation.get("url") else {
                                            return Err(Error::FailedToParseAPIResponse(s.to_string()));
                                        };
                                        web_search_results.push(WebSearchResult {
                                            title: Some(title.to_string()),
                                            summary: None,
                                            content: None,
                                            url: Some(url.to_string()),
                                        });
                                    }
                                }
                            }

                            response = format!("{response}{s}");
                        }
                    },
                    "reasoning" => {  // ["summary"][i]["text"] or ["content"][i]["text"]
                        let reasonings = match (output.get("summary"), output.get("content")) {
                            (_, Some(Value::Array(reasonings))) | (Some(Value::Array(reasonings)), _) => reasonings,
                            _ => {
                                return Err(Error::FailedToParseAPIResponse(s.to_string()));
                            },
                        };

                        if let Some(reasoning) = reasonings.get(0) {
                            if let Some(Value::String(s)) = reasoning.get("text") {
                                thinking = Some(s.to_string());
                            }

                            else {
                                return Err(Error::FailedToParseAPIResponse(s.to_string()));
                            }
                        }
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
            web_search_results,
            cached_input_tokens: raw_response.usage.input_tokens_details.cached_tokens,
            input_tokens: raw_response.usage.input_tokens,
            output_tokens: raw_response.usage.output_tokens,
            log: ApiLog::new(),
        })
    }
}
