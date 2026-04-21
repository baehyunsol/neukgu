use serde::{Deserialize, Serialize};

mod anthropic;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Response {
    pub response: String,
    pub thinking: Option<String>,
    pub web_search_results: Vec<String>,
    pub cached_input_tokens: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}
