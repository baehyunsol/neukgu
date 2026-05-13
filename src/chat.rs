use chrono::Local;
use crate::{Error, LLMToken, Model};
use flate2::Compression;
use flate2::read::{GzDecoder, GzEncoder};
use ragit_fs::{
    WriteMode,
    join,
    join3,
    read_bytes,
    read_dir,
    write_bytes,
    write_string,
};
use serde::{Deserialize, Serialize};
use std::io::Read;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ChatId(u64);

impl ChatId {
    pub fn new() -> ChatId {
        ChatId(rand::random::<u64>())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ChatTurnId(u64);

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Chat {
    pub id: ChatId,
    pub title: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
    pub turns: Vec<ChatTurnId>,
}

impl Chat {
    pub fn new(title: Option<String>) -> Chat {
        let now = Local::now().timestamp_millis();

        Chat {
            id: ChatId::new(),
            title,
            started_at: now,
            updated_at: now,
            turns: vec![],
        }
    }

    pub fn load(id: ChatId, global_index_dir: &str) -> Result<Chat, Error> {
        let chat_at = join3(global_index_dir, "chats", &format!("{:016x}.json", id.0))?;
        let chat = read_bytes(&chat_at)?;
        let chat: Chat = serde_json::from_slice(&chat)?;
        Ok(chat)
    }

    pub fn store(&self, global_index_dir: &str) -> Result<(), Error> {
        let chat_at = join3(global_index_dir, "chats", &format!("{:016x}.json", self.id.0))?;
        write_string(
            &chat_at,
            &serde_json::to_string_pretty(self)?,
            WriteMode::Atomic,
        )?;
        Ok(())
    }

    pub async fn add_turn(&mut self, query: Vec<LLMToken>) -> Result<(), Error> {
        todo!()
    }
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

impl ChatTurn {
    pub fn load(id: ChatTurnId, global_index_dir: &str) -> Result<ChatTurn, Error> {
        let path = join3(global_index_dir, "chat-turns", &format!("{:016x}.json.gz", id.0))?;
        let json = read_bytes(&path)?;
        let mut decompressed = vec![];
        let mut gz = GzDecoder::new(&json[..]);
        gz.read_to_end(&mut decompressed)?;
        Ok(serde_json::from_slice(&decompressed)?)
    }

    pub fn store(&self, global_index_dir: &str) -> Result<(), Error> {
        let json = serde_json::to_vec(self)?;
        let mut compressed = vec![];
        let mut gz = GzEncoder::new(&json[..], Compression::new(3));
        gz.read_to_end(&mut compressed)?;

        Ok(write_bytes(
            &join3(global_index_dir, "chat-turns", &format!("{:016x}.json.gz", self.id.0))?,
            &compressed,
            WriteMode::Atomic,
        )?)
    }
}

pub fn init_chat(global_index_dir: &str, title: Option<String>) -> Result<ChatId, Error> {
    let chat = Chat::new(title);
    chat.store(global_index_dir)?;
    Ok(chat.id)
}

pub fn load_all_chats(global_index_dir: &str) -> Result<Vec<Chat>, Error> {
    let chats_at = join(global_index_dir, "chats")?;
    let mut chats = vec![];

    for chat in read_dir(&chats_at, false)? {
        let chat = read_bytes(&chat)?;
        let chat: Chat = serde_json::from_slice(&chat)?;
        chats.push(chat);
    }

    Ok(chats)
}
