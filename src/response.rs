mod anthropic;

pub struct Response {
    pub response: String,
    pub thinking: Option<String>,
    pub web_search_results: Vec<String>,
    pub input_tokens: u32,
    pub output_tokens: u32,
}
