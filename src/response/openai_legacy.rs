use super::Response;
use crate::{Error, ApiLog};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct OpenaiLegacyResponse {
    choices: Vec<OpenaiLegacyChoice>,
    usage: Option<OpenaiLegacyUsage>,
}

#[derive(Deserialize, Serialize)]
pub struct OpenaiLegacyChoice {
    message: OpenaiLegacyMessage,
}

#[derive(Deserialize, Serialize)]
pub struct OpenaiLegacyMessage {
    content: Option<String>,
    reasoning_content: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct OpenaiLegacyUsage {
    prompt_tokens: Option<u64>,
    completion_tokens: Option<u64>,
    total_tokens: Option<u64>,
    prompt_tokens_details: Option<OpenaiLegacyPromptTokensDetails>,
}

#[derive(Deserialize, Serialize)]
pub struct OpenaiLegacyPromptTokensDetails {
    cached_tokens: Option<u64>,
}

impl Response {
    pub fn from_openai_legacy(s: &str) -> Result<Response, Error> {
        let raw_response: OpenaiLegacyResponse = serde_json::from_str(s)?;
        let Some(choice) = raw_response.choices.first() else {
            return Err(Error::FailedToParseAPIResponse(s.to_string()));
        };

        let response = choice.message.content.clone().unwrap_or_default();
        let thinking = choice.message.reasoning_content.clone();
        let usage = raw_response.usage;

        let cached_input_tokens = usage.as_ref()
            .and_then(|u| u.prompt_tokens_details.as_ref())
            .and_then(|d| d.cached_tokens)
            .unwrap_or(0);
        let input_tokens = usage.as_ref().and_then(|u| u.prompt_tokens).unwrap_or(0);
        let output_tokens = usage.as_ref().and_then(|u| u.completion_tokens).unwrap_or_else(|| {
            usage.as_ref()
                .and_then(|u| u.total_tokens)
                .unwrap_or(input_tokens)
                .saturating_sub(input_tokens)
        });

        Ok(Response {
            response,
            thinking,
            web_search_results: vec![],
            cached_input_tokens,
            input_tokens,
            output_tokens,
            log: ApiLog::new(),
        })
    }
}
