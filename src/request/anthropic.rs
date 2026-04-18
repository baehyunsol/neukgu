use super::{HttpRequest, LLMToken, Request, Thinking};
use base64::Engine;
use crate::Error;
use ragit_fs::read_bytes;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Deserialize, Serialize)]
pub struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,

    // These have to be manually constructed.
    messages: Vec<Value>,
    tools: Vec<Value>,

    thinking: Value,
}

impl Request {
    pub fn to_anthropic_request(&self) -> Result<HttpRequest, Error> {
        let mut headers: HashMap<&str, &str> = HashMap::new();
        let api_key = match std::env::var("ANTHROPIC_API_KEY") {
            Ok(k) => k.to_string(),
            Err(_) => {
                return Err(Error::ApiKeyNotFound { env_var: String::from("ANTHROPIC_API_KEY") });
            },
        };

        headers.insert("x-api-key", &api_key);
        headers.insert("anthropic-version", "2023-06-01");
        headers.insert("content-type", "application/json");
        let headers: HashMap<String, String> = headers.iter().map(
            |(k, v)| (k.to_string(), v.to_string())
        ).collect();

        let mut messages = Vec::with_capacity(self.history.len() * 2 + 1);

        for h in self.history.iter() {
            messages.push(json!({
                "role": "user",
                "content": contents_to_json(&h.query)?,
            }));
            messages.push(json!({
                "role": "assistant",
                "content": &h.response,
            }));
        }

        messages.push(json!({
            "role": "user",
            "content": contents_to_json(&self.query)?,
        }));

        let tools = if self.enable_web_search {
            vec![
                json!({
                    "type": "web_search_20260209",
                    "name": "web_search",
                    "max_uses": 5,
                }),
            ]
        } else {
            vec![]
        };

        let thinking = match self.thinking {
            // TODO: make budget_tokens configurable
            Thinking::Enabled => json!({ "type": "enabled", "budget_tokens": 1024, "display": "summarized" }),
            Thinking::Disabled => json!({ "type": "disabled" }),
            Thinking::Adaptive => json!({ "type": "adaptive", "display": "summarized" }),
        };

        let body = AnthropicRequest {
            model: self.model.name.to_string(),
            max_tokens: 32768,
            system: self.system_prompt.to_string(),
            messages,
            tools,
            thinking,
        };

        let mut body = serde_json::to_value(&body)?;

        match &mut body {
            Value::Object(obj) => {
                // By default neukgu uses its own tool-system, which is not compatible with anthropic's.
                if !self.enable_web_search {
                    obj.insert(String::from("tool_choice"), json!({ "type": "none" }));
                }

                obj.insert(String::from("cache_control"), json!({ "type": "ephemeral" }));
            },
            _ => unreachable!(),
        }

        Ok(HttpRequest {
            url: String::from("https://api.anthropic.com/v1/messages"),
            headers,
            body,
        })
    }
}

fn contents_to_json(contents: &[LLMToken]) -> Result<Value, Error> {
    let mut result = Vec::with_capacity(contents.len());

    for part in contents.iter() {
        let part = match part {
            LLMToken::String(s) => json!({
                "type": "text",
                "text": s,
            }),
            LLMToken::Image(id) => {
                let bytes = read_bytes(&id.path()?)?;
                let image_base64 = encode_base64(&bytes);

                json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": image_base64,
                    },
                })
            },
        };

        result.push(part);
    }

    Ok(result.into())
}

pub fn encode_base64(bytes: &[u8]) -> String {
    base64::prelude::BASE64_STANDARD.encode(bytes)
}
