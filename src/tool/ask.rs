use crate::{
    ApiProvider,
    Error,
    Logger,
    LogEntry,
    Request,
    StringOrImage,
    Thinking,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum AskTo {
    User,
    Web,
}

pub async fn ask_question_to_web(q: &str, logger: &mut Logger) -> Result<String, Error> {
    let mut request = Request {
        // TODO: make it configurable
        model: String::from("claude-sonnet-4-6"),
        provider: ApiProvider::Anthropic,
        system_prompt: String::from("Search web and answer the user question."),
        history: vec![],
        query: vec![StringOrImage::String(q.to_string())],
        enable_web_search: true,
        thinking: Thinking::Disabled,
    };

    logger.log(LogEntry::AskQuestionToWebBegin(q.to_string()))?;
    let response = request.request(logger).await?;
    logger.log(LogEntry::AskQuestionToWebEnd)?;
    Ok(response.response.to_string())
}
