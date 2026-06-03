use chrono::Local;
use crate::{
    ApiLog,
    Error,
    EtcModels,
    Logger,
    LLMToken,
    Model,
    WebSearchResult,
    get_global_index_dir,
    init_log_dir,
    stringify_llm_tokens,
};
use crate::request::{self, Config as RequestConfig, Request, Thinking};
use flate2::Compression;
use flate2::read::{GzDecoder, GzEncoder};
use ragit_fs::{
    WriteMode,
    basename,
    create_dir,
    join,
    join3,
    join4,
    read_bytes,
    read_dir,
    remove_dir_all,
    write_bytes,
    write_string,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ChatId(pub u64);

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
    pub config: Config,
    pub turns: Vec<ChatTurnId>,
    pub unfinished_chat: Option<Vec<LLMToken>>,
}

impl Chat {
    pub fn new(title: Option<String>, config: Config) -> Chat {
        let now = Local::now().timestamp_millis();

        Chat {
            id: ChatId::new(),
            title,
            started_at: now,
            updated_at: now,
            config,
            turns: vec![],
            unfinished_chat: None,
        }
    }

    pub fn load(id: ChatId, global_index_dir: &str) -> Result<Chat, Error> {
        let chat_at = join4(global_index_dir, "chats", &format!("{:016x}", id.0), "context.json")?;
        let chat = read_bytes(&chat_at)?;
        let chat: Chat = serde_json::from_slice(&chat)?;
        Ok(chat)
    }

    pub fn store(&self, global_index_dir: &str) -> Result<(), Error> {
        let chat_at = join4(global_index_dir, "chats", &format!("{:016x}", self.id.0), "context.json")?;
        write_string(
            &chat_at,
            &serde_json::to_string_pretty(self)?,
            WriteMode::Atomic,
        )?;
        Ok(())
    }

    pub async fn add_turn(&mut self, query: Vec<LLMToken>, fallback_api_keys: HashMap<String, String>, global_index_dir: &str) -> Result<(), Error> {
        self.unfinished_chat = Some(query.clone());
        self.store(global_index_dir)?;

        let mut turns = Vec::with_capacity(self.turns.len());
        let user_at = Local::now().timestamp_millis();

        for turn_id in self.turns.iter() {
            turns.push(ChatTurn::load(*turn_id, self.id, global_index_dir)?);
        }

        // We'll use this directory as a working directory.
        // We need this directory in order to store images.
        let working_dir = join3(global_index_dir, "chats", &format!("{:016x}", self.id.0))?;
        let logger = Logger::new(
            join3(&working_dir, ".neukgu", "logs")?,
            Some(join4(global_index_dir, "chats", ".neukgu", "logs")?),
            false,
            true,
        );
        let request = Request {
            model: self.config.model,
            system_prompt: self.config.system_prompt.to_string(),
            history: turns.into_iter().map(|turn| request::Turn { query: turn.user, response: turn.assistant }).collect(),
            query: query.clone(),
            enable_web_search: self.config.enable_web_search,
            thinking: self.config.thinking,
        };

        let mut request_config = self.config.request_config(fallback_api_keys);
        request_config.max_retry = 0;

        let response = request.request(&request_config, &working_dir, &logger).await?;
        let new_turn = ChatTurn {
            chat: self.id,
            id: ChatTurnId::new(),
            model: self.config.model,
            user: query,
            user_at,
            thinking: response.thinking.clone(),
            web_search_results: response.web_search_results.clone(),
            assistant: response.response,
            assistant_at: Local::now().timestamp_millis(),
            api: response.log,
        };
        new_turn.store(global_index_dir)?;

        self.turns.push(new_turn.id);
        self.updated_at = new_turn.assistant_at;
        self.unfinished_chat = None;
        self.store(global_index_dir)?;
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
    pub web_search_results: Vec<WebSearchResult>,
    pub assistant: String,
    pub assistant_at: i64,
    pub api: ApiLog,
}

impl ChatTurn {
    pub fn load(id: ChatTurnId, chat_id: ChatId, global_index_dir: &str) -> Result<ChatTurn, Error> {
        let path = join4(global_index_dir, "chats", &format!("{:016x}", chat_id.0), &format!("{:016x}.json.gz", id.0))?;
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
            &join4(global_index_dir, "chats", &format!("{:016x}", self.chat.0), &format!("{:016x}.json.gz", self.id.0))?,
            &compressed,
            WriteMode::Atomic,
        )?)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub model: Model,
    pub thinking: Thinking,
    pub enable_web_search: bool,
    pub etc_models: EtcModels,
    pub system_prompt: String,
}

impl Config {
    pub fn request_config(&self, fallback_api_keys: HashMap<String, String>) -> RequestConfig {
        RequestConfig {
            fallback_api_keys,
            etc_models: self.etc_models.clone(),
            ..RequestConfig::default()
        }
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            model: Model::Gpt,
            thinking: Thinking::Enabled,
            enable_web_search: false,
            etc_models: EtcModels::default(),
            system_prompt: String::from("You're a kind AI chatbot."),
        }
    }
}

pub async fn add_chat_turn(chat_id: ChatId, fallback_api_keys: HashMap<String, String>, query: Vec<LLMToken>, global_index_dir: &str) -> Result<(), Error> {
    let mut chat = Chat::load(chat_id, global_index_dir)?;
    chat.add_turn(query, fallback_api_keys, global_index_dir).await?;
    Ok(())
}

pub fn delete_chat(chat_id: ChatId, global_index_dir: &str) -> Result<(), Error> {
    Ok(remove_dir_all(&join3(global_index_dir, "chats", &format!("{:016x}", chat_id.0))?)?)
}

pub fn init_chat(title: Option<String>, config: Config, global_index_dir: &str) -> Result<ChatId, Error> {
    let chat = Chat::new(title, config);
    let chat_at = join3(global_index_dir, "chats", &format!("{:016x}", chat.id.0))?;
    create_dir(&chat_at)?;

    // We need these directories to store images and logs.
    create_dir(&join(&chat_at, ".neukgu")?)?;
    create_dir(&join3(&chat_at, ".neukgu", "images")?)?;
    create_dir(&join3(&chat_at, ".neukgu", "interruptions")?)?;
    init_log_dir(&join3(&chat_at, ".neukgu", "logs")?)?;

    chat.store(global_index_dir)?;
    Ok(chat.id)
}

pub fn load_all_chats(global_index_dir: &str) -> Result<Vec<Chat>, Error> {
    let chats_at = join(global_index_dir, "chats")?;
    let mut chats = vec![];

    for chat_id in read_dir(&chats_at, false)? {
        if basename(&chat_id)? == ".neukgu" {
            continue;
        }

        let chat = read_bytes(&join(&chat_id, "context.json")?)?;
        let chat: Chat = serde_json::from_slice(&chat)?;
        chats.push(chat);
    }

    Ok(chats)
}

#[derive(Clone, Debug)]
pub struct MatchPreview {
    pub pre_truncated: bool,
    pub pre: String,
    pub matched: String,
    pub post_truncated: bool,
    pub post: String,
}

impl MatchPreview {
    pub fn new(s: &str, start: usize, end: usize) -> MatchPreview {
        let bytes = s.as_bytes();

        MatchPreview {
            pre_truncated: start >= 20,
            pre: if start < 20 {
                String::from_utf8_lossy(&bytes[..start]).to_string()
            } else {
                String::from_utf8_lossy(&bytes[(start - 16)..start]).to_string()
            }.replace("\n", " "),
            matched: String::from_utf8_lossy(&bytes[start..end]).replace("\n", " "),
            post_truncated: end + 20 < s.len(),
            post: if end + 20 >= s.len() {
                String::from_utf8_lossy(&bytes[end..]).to_string()
            } else {
                String::from_utf8_lossy(&bytes[end..(end + 16)]).to_string()
            }.replace("\n", " "),
        }
    }
}

pub fn find_pattern_in_chats(pattern: &str) -> Result<Vec<(ChatId, Vec<MatchPreview>)>, Error> {
    let mut result = vec![];
    let pattern = regex::Regex::new(pattern)?;
    let global_index_dir = get_global_index_dir()?;
    let chats = load_all_chats(&global_index_dir)?;

    for chat in chats.iter() {
        let mut previews = vec![];

        for turn in chat.turns.iter() {
            let turn = ChatTurn::load(*turn, chat.id, &global_index_dir)?;
            let user = stringify_llm_tokens(&turn.user);

            if let Some(cap) = pattern.captures(&user) {
                let m = cap.get(0).unwrap();
                previews.push(MatchPreview::new(&user, m.start(), m.end()));
            }

            if let Some(cap) = pattern.captures(&turn.assistant) {
                let m = cap.get(0).unwrap();
                previews.push(MatchPreview::new(&turn.assistant, m.start(), m.end()));
            }
        }

        if !previews.is_empty() {
            result.push((chat.id, previews));
        }
    }

    Ok(result)
}
