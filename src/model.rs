use crate::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum Model {
    GptMini,
    Gpt,
    OpenaiEtc1,
    OpenaiEtc2,
    OpenaiEtc3,
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
    pub fn api_name(&self) -> &'static str {
        match self {
            Model::GptMini => "gpt-5.4-mini",
            Model::Gpt => "gpt-5.5",
            Model::OpenaiEtc1 => unreachable!(),
            Model::OpenaiEtc2 => unreachable!(),
            Model::OpenaiEtc3 => unreachable!(),
            Model::Haiku => "claude-haiku-4-5",
            Model::Sonnet => "claude-sonnet-4-6",
            Model::Opus => "claude-opus-4-7",
            Model::Mock => "mock",
            Model::GeminiPro => "gemini-3.1-pro-preview",
            Model::GeminiFlash => "gemini-3-flash-preview",
            Model::Disabled => "disabled",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            Model::GptMini => "gpt-mini",
            Model::Gpt => "gpt",
            Model::OpenaiEtc1 => "openai-etc1",
            Model::OpenaiEtc2 => "openai-etc2",
            Model::OpenaiEtc3 => "openai-etc3",
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

    pub fn api_key_env_var(&self) -> &'static str {
        match self {
            Model::GptMini => "OPENAI_API_KEY",
            Model::Gpt => "OPENAI_API_KEY",
            Model::OpenaiEtc1 => "OPENAI_ETC1_API_KEY",
            Model::OpenaiEtc2 => "OPENAI_ETC2_API_KEY",
            Model::OpenaiEtc3 => "OPENAI_ETC3_API_KEY",
            Model::Haiku => "ANTHROPIC_API_KEY",
            Model::Sonnet => "ANTHROPIC_API_KEY",
            Model::Opus => "ANTHROPIC_API_KEY",
            Model::Mock => "MOCK_API_KEY",
            Model::GeminiPro => "GEMINI_API_KEY",
            Model::GeminiFlash => "GEMINI_API_KEY",
            Model::Disabled => "_",
        }
    }

    pub fn from_short_name(s: &str) -> Result<Model, Error> {
        match s {
            "gpt-mini" => Ok(Model::GptMini),
            "gpt" => Ok(Model::Gpt),
            "openai-etc1" => Ok(Model::OpenaiEtc1),
            "openai-etc2" => Ok(Model::OpenaiEtc2),
            "openai-etc3" => Ok(Model::OpenaiEtc3),
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
            Model::OpenaiEtc1 => false,
            Model::OpenaiEtc2 => false,
            Model::OpenaiEtc3 => false,
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
            Model::GptMini => ApiProvider::Openai,
            Model::Gpt => ApiProvider::Openai,
            Model::OpenaiEtc1 => ApiProvider::OpenaiLegacy,
            Model::OpenaiEtc2 => ApiProvider::OpenaiLegacy,
            Model::OpenaiEtc3 => ApiProvider::OpenaiLegacy,
            Model::Haiku => ApiProvider::Anthropic,
            Model::Sonnet => ApiProvider::Anthropic,
            Model::Opus => ApiProvider::Anthropic,
            Model::Mock => ApiProvider::Mock,
            Model::GeminiPro => ApiProvider::Gemini,
            Model::GeminiFlash => ApiProvider::Gemini,
            Model::Disabled => ApiProvider::Mock,
        }
    }

    pub fn all() -> [Model; 12] {
        [
            Model::GptMini,
            Model::Gpt,
            Model::OpenaiEtc1,
            Model::OpenaiEtc2,
            Model::OpenaiEtc3,
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

impl Agents {
    pub fn any<F>(&self, f: F) -> bool where F: Fn(Model) -> bool {
        f(self.big) || f(self.small) || f(self.search) || f(self.summary)
    }

    pub fn iter(&self) -> impl Iterator<Item=Model> {
        use std::iter::once;
        once(self.big).chain(once(self.small)).chain(once(self.search)).chain(once(self.summary))
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
    Openai,
    OpenaiLegacy,
    Mock,
    Gemini,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct EtcModels {
    pub openai_etc1_base_url: Option<String>,
    pub openai_etc1_model: Option<String>,
    pub openai_etc2_base_url: Option<String>,
    pub openai_etc2_model: Option<String>,
    pub openai_etc3_base_url: Option<String>,
    pub openai_etc3_model: Option<String>,
}

impl Default for EtcModels {
    fn default() -> EtcModels {
        EtcModels {
            openai_etc1_base_url: None,
            openai_etc1_model: None,
            openai_etc2_base_url: None,
            openai_etc2_model: None,
            openai_etc3_base_url: None,
            openai_etc3_model: None,
        }
    }
}
