mod anthropic;

pub struct Response {
    pub response: String,
    pub thinking: Option<String>,
    pub web_search_results: Vec<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
}
