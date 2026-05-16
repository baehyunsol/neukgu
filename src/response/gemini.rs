use super::{Response, WebSearchResult};
use crate::{ApiLog, Error};
use serde_json::Value;

impl Response {
    pub fn from_gemini(s: &str) -> Result<Response, Error> {
        let raw: Value = serde_json::from_str(s)?;

        let mut response = String::new();
        let mut thinking: Option<String> = None;
        let mut web_search_results = vec![];

        // Parse candidates[0].content.parts
        let parts = match raw
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("content"))
            .and_then(|c| c.get("parts"))
        {
            Some(Value::Array(parts)) => parts,
            _ => {
                return Err(Error::FailedToParseAPIResponse(s.to_string()));
            },
        };

        for part in parts.iter() {
            // thought parts have "thought": true
            let is_thought = matches!(part.get("thought"), Some(Value::Bool(true)));

            if let Some(Value::String(text)) = part.get("text") {
                if is_thought {
                    // Accumulate thinking text
                    match &mut thinking {
                        Some(existing) => {
                            existing.push_str(text);
                        },
                        None => {
                            thinking = Some(text.clone());
                        },
                    }
                } else {
                    response.push_str(text);
                }
            }
            // executableCode / codeExecutionResult parts — ignore
        }

        // Parse grounding metadata for web search results
        // candidates[0].groundingMetadata.groundingChunks[].web
        if let Some(chunks) = raw
            .get("candidates")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("groundingMetadata"))
            .and_then(|m| m.get("groundingChunks"))
            .and_then(|c| c.as_array())
        {
            for chunk in chunks.iter() {
                if let Some(web) = chunk.get("web") {
                    let title = web.get("title").and_then(|v| v.as_str()).map(|s| s.to_string());
                    let url = web.get("uri").and_then(|v| v.as_str()).map(|s| s.to_string());
                    web_search_results.push(WebSearchResult {
                        title,
                        summary: None,
                        content: None,
                        url,
                    });
                }
            }
        }

        // Parse token usage from usageMetadata
        let usage = raw.get("usageMetadata");
        let cached_input_tokens = usage
            .and_then(|u| u.get("cachedContentTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let input_tokens = usage
            .and_then(|u| u.get("promptTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let output_tokens = usage
            .and_then(|u| u.get("candidatesTokenCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        Ok(Response {
            response,
            thinking,
            web_search_results,
            cached_input_tokens,
            input_tokens,
            output_tokens,
            log: ApiLog::new(),
        })
    }
}
