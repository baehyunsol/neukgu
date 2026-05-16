use crate::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Model {
    GptMini,
    Gpt,
    OpenAiComp,
    Haiku,
    Sonnet,
    Opus,
    Mock,
    GeminiPro,
    GeminiFlash,

    // You can disable certain agents by selecting this model!
    Disabled,
}

impl Model {
    pub fn api_name(&self) -> String {
        match self {
            Model::GptMini => "gpt-5.4-mini".to_string(),
            Model::Gpt => "gpt-5.5".to_string(),
            Model::OpenAiComp => std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-5.5".to_string()),
            Model::Haiku => "claude-haiku-4-5".to_string(),
            Model::Sonnet => "claude-sonnet-4-6".to_string(),
            Model::Opus => "claude-opus-4-7".to_string(),
            Model::Mock => "mock".to_string(),
            Model::GeminiPro => "gemini-3.1-pro-preview".to_string(),
            Model::GeminiFlash => "gemini-3-flash-preview".to_string(),
            Model::Disabled => "disabled".to_string(),
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Model::GptMini => "gpt-mini",
            Model::Gpt => "gpt",
            Model::OpenAiComp => "openai-compatible",
            Model::Haiku => "haiku",
            Model::Sonnet => "sonnet",
            Model::Opus => "opus",
            Model::Mock => "mock",
            Model::GeminiPro => "gemini-pro",
            Model::GeminiFlash => "gemini-flash",

            // This name makes more sense in the ui.
            Model::Disabled => "disable",
        }
    }

    pub fn from_short_name(s: &str) -> Result<Model, Error> {
        match s {
            "gpt-mini" => Ok(Model::GptMini),
            "gpt" => Ok(Model::Gpt),
            "openai-compatible" => Ok(Model::OpenAiComp),
            "haiku" => Ok(Model::Haiku),
            "sonnet" => Ok(Model::Sonnet),
            "opus" => Ok(Model::Opus),
            "mock" => Ok(Model::Mock),
            "disable" => Ok(Model::Disabled),
            "gemini-pro" => Ok(Model::GeminiPro),
            "gemini-flash" => Ok(Model::GeminiFlash),
            _ => Err(Error::InvalidModelName(s.to_string())),
        }
    }

    pub fn supports_web_search(&self) -> bool {
        match self {
            Model::GptMini => true,  // TODO: I haven't tested yet
            Model::Gpt => true,
            Model::OpenAiComp => false,
            Model::Haiku => false,  // As of 2026-05-12
            Model::Sonnet => true,
            Model::Opus => true,
            Model::Mock => false,
            Model::GeminiPro => true,
            Model::GeminiFlash => true,
            Model::Disabled => true,
        }
    }

    pub fn provider(&self) -> ApiProvider {
        match self {
            Model::GptMini => ApiProvider::OpenAi,
            Model::Gpt => ApiProvider::OpenAi,
            Model::OpenAiComp => ApiProvider::OpenAiComp,
            Model::Haiku => ApiProvider::Anthropic,
            Model::Sonnet => ApiProvider::Anthropic,
            Model::Opus => ApiProvider::Anthropic,
            Model::Mock => ApiProvider::Mock,
            Model::GeminiPro => ApiProvider::Gemini,
            Model::GeminiFlash => ApiProvider::Gemini,
            Model::Disabled => ApiProvider::Mock,
        }
    }

    pub fn all() -> [Model; 10] {
        [
            Model::GptMini,
            Model::Gpt,
            Model::OpenAiComp,
            Model::Haiku,
            Model::Sonnet,
            Model::Opus,
            Model::Mock,
            Model::GeminiPro,
            Model::GeminiFlash,
            Model::Disabled,
        ]
    }

    pub fn short_names() -> Vec<&'static str> {
        Model::all().iter().map(|m| m.short_name()).collect()
    }
}

impl fmt::Display for Model {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{}", self.short_name())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Agents {
    pub big: Model,
    pub small: Model,  // WIP
    pub search: Model,
    pub summary: Model,
}

impl Agents {
    pub fn single(model: Model) -> Agents {
        Agents {
            big: model,
            small: model,
            search: model,
            summary: model,
        }
    }
}

impl Default for Agents {
    fn default() -> Agents {
        Agents {
            big: Model::Sonnet,
            small: Model::Haiku,
            search: Model::Gpt,
            summary: Model::Haiku,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub enum ApiProvider {
    Anthropic,
    OpenAi,
    OpenAiComp,
    Mock,
    Gemini,
}
