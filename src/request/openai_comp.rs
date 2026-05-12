use super::{HttpRequest, LLMToken, Request};
use base64::Engine;
use crate::Error;
use ragit_fs::read_bytes;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

// chat-completion api
#[derive(Deserialize, Serialize)]
pub struct OpenAiCompRequest {
    model: String,
    messages: Vec<Value>,
}

impl Request {
    pub fn to_openai_comp_request(&self, working_dir: &str) -> Result<HttpRequest, Error> {
        let mut headers: HashMap<&str, String> = HashMap::new();

        if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
            headers.insert("Authorization", format!("Bearer {api_key}"));
        }

        let headers: HashMap<String, String> = headers.iter().map(
            |(k, v)| (k.to_string(), v.to_string())
        ).collect();
        let mut messages = Vec::with_capacity(self.history.len() * 2 + 2);

        if !self.system_prompt.is_empty() {
            messages.push(json!({
                "role": "system",
                "content": self.system_prompt,
            }));
        }

        for h in self.history.iter() {
            messages.push(json!({
                "role": "user",
                "content": contents_to_json(&h.query, working_dir)?,
            }));
            messages.push(json!({
                "role": "assistant",
                "content": &h.response,
            }));
        }

        messages.push(json!({
            "role": "user",
            "content": contents_to_json(&self.query, working_dir)?,
        }));

        let body = OpenAiCompRequest {
            model: self.model.api_name().to_string(),
            messages,
        };

        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| String::from("https://api.openai.com/v1"))
            .trim_end_matches('/')
            .to_string();

        Ok(HttpRequest {
            url: format!("{base_url}/chat/completions"),
            headers,
            body: serde_json::to_value(&body)?,
        })
    }
}

fn contents_to_json(contents: &[LLMToken], working_dir: &str) -> Result<Value, Error> {
    if contents.len() == 1 {
        if let LLMToken::String(s) = &contents[0] {
            return Ok(Value::String(s.to_string()));
        }
    }

    let mut result = Vec::with_capacity(contents.len());

    for part in contents.iter() {
        let part = match part {
            LLMToken::String(s) => json!({
                "type": "text",
                "text": s,
            }),
            LLMToken::Image(id) => {
                let bytes = read_bytes(&id.path(working_dir)?)?;
                let image_base64 = encode_base64(&bytes);

                json!({
                    "type": "image_url",
                    "image_url": {
                        "url": format!("data:image/png;base64,{image_base64}"),
                    },
                })
            },
        };

        result.push(part);
    }

    Ok(result.into())
}

fn encode_base64(bytes: &[u8]) -> String {
    base64::prelude::BASE64_STANDARD.encode(bytes)
}
