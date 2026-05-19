use super::{Config, HttpRequest, LLMToken, Request, Thinking};
use base64::Engine;
use crate::Error;
use ragit_fs::read_bytes;
use serde_json::{Value, json};
use std::collections::HashMap;

impl Request {
    pub fn to_gemini_request(&self, config: &Config, working_dir: &str) -> Result<HttpRequest, Error> {
        let api_key_env_var = self.model.api_key_env_var();
        let api_key = match std::env::var(api_key_env_var) {
            Ok(k) => k.to_string(),
            Err(_) => match config.fallback_api_keys.get(api_key_env_var) {
                Some(k) => k.to_string(),
                None => return Err(Error::ApiKeyNotFound { env_var: String::from(api_key_env_var) }),
            },
        };

        let headers: HashMap<String, String> = HashMap::new();
        // Gemini uses the API key as a query parameter, not a header

        let mut contents: Vec<Value> = Vec::with_capacity(self.history.len() * 2 + 1);

        for h in self.history.iter() {
            contents.push(json!({
                "role": "user",
                "parts": tokens_to_parts(&h.query, working_dir)?,
            }));
            contents.push(json!({
                "role": "model",
                "parts": [{ "text": &h.response }],
            }));
        }

        contents.push(json!({
            "role": "user",
            "parts": tokens_to_parts(&self.query, working_dir)?,
        }));

        // Tools
        let tools: Vec<Value> = if self.enable_web_search {
            vec![json!({ "google_search": {} })]
        } else {
            vec![]
        };

        // Thinking config
        let thinking_config = match self.thinking {
            Thinking::Enabled => json!({ "thinkingBudget": 8192 }),
            Thinking::Disabled => json!({ "thinkingBudget": 1024 }),  // gemini doesn't work with 0 budget
            Thinking::Adaptive => json!({}),  // Let Gemini decide
        };

        let mut body = json!({
            "system_instruction": {
                "parts": [{ "text": &self.system_prompt }],
            },
            "contents": contents,
            "generationConfig": {
                "maxOutputTokens": 32768,
                "thinkingConfig": thinking_config,
            },
        });

        if !tools.is_empty() {
            if let Value::Object(ref mut obj) = body {
                obj.insert(String::from("tools"), Value::Array(tools));
            }
        }

        let model_name = self.model.api_name();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model_name,
            api_key,
        );

        Ok(HttpRequest {
            url,
            headers,
            body,
        })
    }
}

fn tokens_to_parts(tokens: &[LLMToken], working_dir: &str) -> Result<Vec<Value>, Error> {
    let mut parts = Vec::with_capacity(tokens.len());

    for token in tokens.iter() {
        let part = match token {
            LLMToken::String(s) => json!({ "text": s }),
            LLMToken::Image(id) => {
                let bytes = read_bytes(&id.path(working_dir)?)?;
                let image_base64 = encode_base64(&bytes);
                json!({
                    "inlineData": {
                        "mimeType": "image/png",
                        "data": image_base64,
                    },
                })
            },
        };

        parts.push(part);
    }

    Ok(parts)
}

fn encode_base64(bytes: &[u8]) -> String {
    base64::prelude::BASE64_STANDARD.encode(bytes)
}
