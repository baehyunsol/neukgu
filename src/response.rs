use crate::ApiLog;
use serde::{Deserialize, Serialize};

mod anthropic;
mod openai;
mod openai_comp;
mod gemini;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Response {
    pub response: String,
    pub thinking: Option<String>,
    pub web_search_results: Vec<WebSearchResult>,
    pub cached_input_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub log: ApiLog,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct WebSearchResult {
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub url: Option<String>,
}
