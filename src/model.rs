use crate::Error;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Model {
    GptMini,
    Gpt,
    Haiku,
    Sonnet,
    Opus,
    Mock,

    // You can disable certain agents by selecting this model!
    Disabled,
}

impl Model {
    pub fn api_name(&self) -> &'static str {
        match self {
            Model::GptMini => "gpt-5.4-mini",
            Model::Gpt => "gpt-5.5",
            Model::Haiku => "claude-haiku-4-5",
            Model::Sonnet => "claude-sonnet-4-6",
            Model::Opus => "claude-opus-4-7",
            Model::Mock => "mock",
            Model::Disabled => "disabled",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Model::GptMini => "gpt-mini",
            Model::Gpt => "gpt",
            Model::Haiku => "haiku",
            Model::Sonnet => "sonnet",
            Model::Opus => "opus",
            Model::Mock => "mock",

            // This name makes more sense in the ui.
            Model::Disabled => "disable",
        }
    }

    pub fn from_short_name(s: &str) -> Result<Model, Error> {
        match s {
            "gpt-mini" => Ok(Model::GptMini),
            "gpt" => Ok(Model::Gpt),
            "haiku" => Ok(Model::Haiku),
            "sonnet" => Ok(Model::Sonnet),
            "opus" => Ok(Model::Opus),
            "mock" => Ok(Model::Mock),
            "disable" => Ok(Model::Disabled),
            _ => Err(Error::InvalidModelName(s.to_string())),
        }
    }

    pub fn provider(&self) -> ApiProvider {
        match self {
            Model::GptMini => ApiProvider::OpenAi,
            Model::Gpt => ApiProvider::OpenAi,
            Model::Haiku => ApiProvider::Anthropic,
            Model::Sonnet => ApiProvider::Anthropic,
            Model::Opus => ApiProvider::Anthropic,
            Model::Mock => ApiProvider::Mock,
            Model::Disabled => ApiProvider::Mock,
        }
    }

    pub fn all() -> [Model; 7] {
        [
            Model::GptMini,
            Model::Gpt,
            Model::Haiku,
            Model::Sonnet,
            Model::Opus,
            Model::Mock,
            Model::Disabled,
        ]
    }

    pub fn short_names() -> Vec<&'static str> {
        Model::all().iter().map(|m| m.short_name()).collect()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
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
    Mock,
}
