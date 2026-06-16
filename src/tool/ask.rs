use super::{Permission, PermissionPreview, ToolCallError, ToolCallSuccess, ToolPermissionKind};
use async_std::task::sleep;
use crate::{
    Config,
    Context,
    Error,
    InterruptId,
    LLMToken,
    Logger,
    LogEntry,
    Request,
    Thinking,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum AskTo {
    User,
    Web,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QuestionToUser {
    pub question: String,
    pub kind: QuestionKind,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum QuestionKind {
    FreeText,
    Choice {
        options: Vec<String>,
        multi: bool,
    },
    ToolPermission {
        kind: ToolPermissionKind,
        path: Option<String>,
        preview: PermissionPreview,
    },
    RunPermission {
        command: Vec<String>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum UserResponse {
    Answer(UserAnswer),
    Timeout,
    Reject,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum UserAnswer {
    FreeText(String),
    SingleChoice(usize),
    MultiChoices(Vec<usize>),
    Permission(Permission),
}

impl fmt::Display for UserAnswer {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            UserAnswer::FreeText(a) => write!(fmt, "{a}"),
            UserAnswer::SingleChoice(n) => write!(fmt, "{n}"),
            UserAnswer::MultiChoices(ns) => write!(fmt, "{}", ns.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")),
            UserAnswer::Permission(p) => write!(fmt, "{p:?}"),
        }
    }
}

pub async fn ask_question_to_web(q: &str, config: &Config, working_dir: &str, logger: &Logger) -> Result<String, Error> {
    let request = Request {
        model: config.agents.search,
        system_prompt: String::from("Search web and answer the user question."),
        history: vec![],
        query: vec![LLMToken::String(q.to_string())],
        enable_web_search: true,
        thinking: Thinking::Disabled,
    };

    logger.log(LogEntry::AskQuestionToWebBegin(q.to_string()))?;
    let response = request.request(&config.request_config(), working_dir, false, logger).await?;
    logger.log(LogEntry::AskQuestionToWebEnd)?;
    Ok(response.response.to_string())
}

pub async fn ask_question_to_user(
    id: InterruptId,
    q: &QuestionToUser,
    context: &mut Context,
    config: &Config,
) -> Result<Result<ToolCallSuccess, ToolCallError>, Error> {
    let response;
    let tool_call_result = 'block: {
        if config.user_response_timeout == 0 {
            response = UserResponse::Reject;
            break 'block Err(ToolCallError::UserRejectedToRespond);
        }

        let started_at = Instant::now();
        context.ask_to_user(id, q)?;

        loop {
            if let Some(response_) = context.check_user_response(id)? {
                response = response_.clone();

                match response_ {
                    UserResponse::Answer(answer) => {
                        break 'block Ok(ToolCallSuccess::Ask { to: AskTo::User, answer });
                    },
                    UserResponse::Timeout => {
                        break 'block Err(ToolCallError::UserNotResponding);
                    },
                    UserResponse::Reject => {
                        break 'block Err(ToolCallError::UserRejectedToRespond);
                    },
                }
            }

            sleep(Duration::from_millis(1_000)).await;
            context.sync_with_fe()?;

            if !context.is_fe_alive()? {
                response = UserResponse::Timeout;
                break 'block Err(ToolCallError::UserNotResponding);
            }

            // It waits 3 more seconds than the set timeout because the frontend is a few seconds slower than the backend.
            if Instant::now().duration_since(started_at.clone()).as_secs() > config.user_response_timeout + 3 {
                break;
            }
        }

        response = UserResponse::Timeout;
        Err(ToolCallError::UserNotResponding)
    };

    context.answer_to_llm(id, q.clone(), response)?;
    Ok(tool_call_result)
}
