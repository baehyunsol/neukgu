use super::{HttpRequest, LLMToken, Request, Thinking};
use base64::Engine;
use crate::Error;
use ragit_fs::read_bytes;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Deserialize, Serialize)]
pub struct OpenaiRequest {
    model: String,
    instructions: String,
    input: Vec<Value>,
    tools: Vec<Value>,
    tool_choice: String,
    reasoning: OpenaiReasoning,
}

#[derive(Deserialize, Serialize)]
pub struct OpenaiReasoning {
    effort: String,
}

impl Request {
    pub fn to_openai_request(&self, working_dir: &str) -> Result<HttpRequest, Error> {
        let mut headers: HashMap<&str, String> = HashMap::new();
        let api_key = match std::env::var("OPENAI_API_KEY") {
            Ok(k) => k.to_string(),
            Err(_) => {
                return Err(Error::ApiKeyNotFound { env_var: String::from("OPENAI_API_KEY") });
            },
        };

        headers.insert("Authorization", format!("Bearer {api_key}"));
        // headers.insert("content-type", String::from("application/json"));
        let headers: HashMap<String, String> = headers.iter().map(
            |(k, v)| (k.to_string(), v.to_string())
        ).collect();

        let mut input = Vec::with_capacity(self.history.len() * 2 + 1);

        for h in self.history.iter() {
            input.push(json!({
                "role": "user",
                "content": contents_to_json(&h.query, working_dir)?,
            }));
            input.push(json!({
                "role": "assistant",
                "content": vec![json!({ "type": "output_text", "text": &h.response })],
            }));
        }

        input.push(json!({
            "role": "user",
            "content": contents_to_json(&self.query, working_dir)?,
        }));

        let (tools, tool_choice) = if self.enable_web_search {
            (vec![json!({ "type": "web_search_preview" })], String::from("auto"))
        } else {
            (vec![], String::from("auto"))
        };

        let reasoning_effort = match self.thinking {
            Thinking::Enabled => "medium",
            Thinking::Disabled => "none",
            // It seems like gpt doesn't support adaptive thinking
            Thinking::Adaptive => "medium",
        };

        let body = OpenaiRequest {
            model: self.model.name.to_string(),
            instructions: self.system_prompt.to_string(),
            input,
            tools,
            tool_choice,
            reasoning: OpenaiReasoning {
                effort: reasoning_effort.to_string(),
            },
        };

        Ok(HttpRequest {
            url: String::from("https://api.openai.com/v1/responses"),
            headers,
            body: serde_json::to_value(&body)?,
        })
    }
}

fn contents_to_json(contents: &[LLMToken], working_dir: &str) -> Result<Value, Error> {
    let mut result = Vec::with_capacity(contents.len());

    for part in contents.iter() {
        let part = match part {
            LLMToken::String(s) => json!({
                "type": "input_text",
                "text": s,
            }),
            LLMToken::Image(id) => {
                let bytes = read_bytes(&id.path(working_dir)?)?;
                let image_base64 = encode_base64(&bytes);

                json!({
                    "type": "input_image",
                    "image_url": format!("data:image/png;base64,{image_base64}"),
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
