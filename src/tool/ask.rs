use crate::{
    Error,
    LLMToken,
    Logger,
    LogEntry,
    Model,
    Request,
    Thinking,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum AskTo {
    User,
    Web,
}

pub async fn ask_question_to_web(q: &str, working_dir: &str, logger: &Logger, model: Model) -> Result<String, Error> {
    let mut request = Request {
        model,
        system_prompt: String::from("Search web and answer the user question."),
        history: vec![],
        query: vec![LLMToken::String(q.to_string())],
        enable_web_search: true,
        thinking: Thinking::Disabled,
    };

    logger.log(LogEntry::AskQuestionToWebBegin(q.to_string()))?;
    let response = request.request(working_dir, logger).await?;
    logger.log(LogEntry::AskQuestionToWebEnd)?;
    Ok(response.response.to_string())
}
