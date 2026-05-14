use chrono::Local;
use crate::{Error, Logger, LLMToken, Model};
use crate::request::{self, Request, Thinking};
use flate2::Compression;
use flate2::read::{GzDecoder, GzEncoder};
use ragit_fs::{
    WriteMode,
    basename,
    join,
    join3,
    read_bytes,
    read_dir,
    remove_file,
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

impl ChatTurnId {
    pub fn new() -> ChatTurnId {
        ChatTurnId(rand::random::<u64>())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Chat {
    pub id: ChatId,
    pub title: Option<String>,
    pub started_at: i64,
    pub updated_at: i64,
    pub config: ChatConfig,
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
            config: ChatConfig::new(),
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

    pub async fn add_turn(&mut self, query: Vec<LLMToken>, global_index_dir: &str) -> Result<(), Error> {
        let mut turns = Vec::with_capacity(self.turns.len());
        let user_at = Local::now().timestamp_millis();

        for turn_id in self.turns.iter() {
            turns.push(ChatTurn::load(*turn_id, global_index_dir)?);
        }

        // We'll use this directory as a working directory.
        // We need this directory in order to store images.
        let working_dir = join(global_index_dir, "chats")?;
        let logger = Logger::new(
            join3(&working_dir, ".neukgu", "logs")?,
            false,
            true,
        );
        let request = Request {
            model: self.config.model,
            system_prompt: String::from("You're a kind chatbot."),
            history: turns.into_iter().map(|turn| request::Turn { query: turn.user, response: turn.assistant }).collect(),
            query: query.clone(),
            enable_web_search: self.config.enable_web_search,
            thinking: self.config.thinking,
        };

        let response = request.request(&working_dir, &logger).await?;
        let new_turn = ChatTurn {
            chat: self.id,
            id: ChatTurnId::new(),
            model: self.config.model,
            user: query,
            user_at,
            thinking: response.thinking.clone(),
            assistant: response.response,
            assistant_at: Local::now().timestamp_millis(),
        };
        new_turn.store(global_index_dir)?;

        self.turns.push(new_turn.id);
        self.updated_at = new_turn.assistant_at;
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatTurn {
    pub chat: ChatId,
    pub id: ChatTurnId,
    pub model: Model,
    pub user: Vec<LLMToken>,
    pub user_at: i64,
    pub thinking: Option<String>,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatConfig {
    pub model: Model,
    pub thinking: Thinking,
    pub enable_web_search: bool,
}

impl ChatConfig {
    pub fn new() -> ChatConfig {
        ChatConfig {
            model: Model::Gpt,
            thinking: Thinking::Enabled,
            enable_web_search: false,
        }
    }
}

pub async fn add_chat_turn(chat_id: ChatId, query: Vec<LLMToken>, global_index_dir: &str) -> Result<(), Error> {
    let mut chat = Chat::load(chat_id, global_index_dir)?;
    chat.add_turn(query, global_index_dir).await?;
    chat.store(global_index_dir)?;
    Ok(())
}

pub fn delete_chat(chat_id: ChatId, global_index_dir: &str) -> Result<(), Error> {
    let chat = Chat::load(chat_id, global_index_dir)?;

    for turn in chat.turns.iter() {
        let turn_at = join3(global_index_dir, "chat-turns", &format!("{:016x}.json.gz", turn.0))?;
        remove_file(&turn_at)?;
    }

    let chat_at = join3(global_index_dir, "chats", &format!("{:016x}.json", chat_id.0))?;
    remove_file(&chat_at)?;
    Ok(())
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
        if basename(&chat)? == ".neukgu" {
            continue;
        }

        let chat = read_bytes(&chat)?;
        let chat: Chat = serde_json::from_slice(&chat)?;
        chats.push(chat);
    }

    Ok(chats)
}
