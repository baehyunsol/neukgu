use super::{Config, HttpRequest, LLMToken, Request, Thinking};
use crate::{Error, Model, encode_base64};
use ragit_fs::read_bytes;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;

// chat-completion api
#[derive(Deserialize, Serialize)]
pub struct OpenaiLegacyRequest {
    model: String,
    messages: Vec<Value>,
    reasoning_effort: String,
}

impl Request {
    pub fn to_openai_legacy_request(&self, config: &Config, working_dir: &str) -> Result<HttpRequest, Error> {
        let mut headers: HashMap<&str, String> = HashMap::new();

        // It requires an API key even for local models. Users can insert an arbitrary api key for local models.
        // Raising this error can teach the user which env var is needed for this model.
        let api_key_env_var = self.model.api_key_env_var();
        let api_key = match std::env::var(api_key_env_var) {
            Ok(k) => k.to_string(),
            Err(_) => match config.fallback_api_keys.get(api_key_env_var) {
                Some(k) => k.to_string(),
                None => return Err(Error::ApiKeyNotFound { env_var: String::from(api_key_env_var) }),
            },
        };

        headers.insert("Authorization", format!("Bearer {api_key}"));
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

        let reasoning_effort = match self.thinking {
            Thinking::Enabled => "high",
            Thinking::Disabled => "none",
            Thinking::Adaptive => "low",
        }.to_string();

        let (model_env_var, base_url_env_var) = match self.model {
            Model::OpenaiEtc1 => ("OPENAI_ETC1_MODEL", "OPENAI_ETC1_BASE_URL"),
            Model::OpenaiEtc2 => ("OPENAI_ETC2_MODEL", "OPENAI_ETC2_BASE_URL"),
            Model::OpenaiEtc3 => ("OPENAI_ETC3_MODEL", "OPENAI_ETC3_BASE_URL"),
            _ => unreachable!(),
        };

        let model = match self.model {
            Model::OpenaiEtc1 => config.openai_etc1_model.clone(),
            Model::OpenaiEtc2 => config.openai_etc2_model.clone(),
            Model::OpenaiEtc3 => config.openai_etc3_model.clone(),
            _ => unreachable!(),
        };
        let model = match (model, std::env::var(model_env_var)) {
            (_, Ok(model)) => model,
            (Some(model), _) => model,
            _ => {
                return Err(Error::ApiKeyNotFound { env_var: model_env_var.to_string() });
            },
        };

        let body = OpenaiLegacyRequest {
            model,
            messages,
            reasoning_effort,
        };

        let base_url = match self.model {
            Model::OpenaiEtc1 => config.openai_etc1_base_url.clone(),
            Model::OpenaiEtc2 => config.openai_etc2_base_url.clone(),
            Model::OpenaiEtc3 => config.openai_etc3_base_url.clone(),
            _ => unreachable!(),
        };
        let base_url = match (base_url, std::env::var(base_url_env_var)) {
            (_, Ok(base_url)) => base_url,
            (Some(base_url), _) => base_url,
            _ => {
                return Err(Error::ApiKeyNotFound { env_var: base_url_env_var.to_string() });
            },
        };
        let base_url = base_url.trim_end_matches('/').to_string();

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
