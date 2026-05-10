use crate::{LLMToken, Model};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ChatId(u64);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ChatTurnId(u64);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Chat {
    pub id: ChatId,
    pub title: String,
    pub started_at: i64,
    pub updated_at: i64,
    pub turns: Vec<ChatTurnId>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatTurn {
    pub chat: ChatId,
    pub id: ChatTurnId,
    pub model: Model,
    pub user: Vec<LLMToken>,
    pub user_at: i64,
    pub assistant: String,
    pub assistant_at: i64,
}
