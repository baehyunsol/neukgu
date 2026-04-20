use crate::{Error, Model};
use ragit_fs::{
    WriteMode,
    join,
    read_string,
    write_string,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub model: String,
    pub sandbox_root: String,
    pub llm_context_max_len: u64,
    pub text_file_max_len: u64,
    pub text_file_max_lines: u64,
    pub pdf_max_pages: u64,
    pub dir_max_entries: u64,
    pub stdout_max_len: u64,
    pub default_command_timeout: u64,

    // I'm worried if AI mistakens millisec and sec.
    pub command_max_timeout: u64,
}

impl Config {
    pub fn model(&self) -> Result<Model, Error> {
        match self.model.as_str() {
            "sonnet" => Ok(Model::sonnet()),
            "mock" => Ok(Model::mock()),
            _ => Err(Error::InvalidModelName(self.model.to_string())),
        }
    }

    pub fn load() -> Result<Self, Error> {
        let s = read_string(&join(".neukgu", "config.json")?)?;
        Ok(serde_json::from_str(&s)?)
    }

    pub fn store(&self) -> Result<(), Error> {
        Ok(write_string(
            &join(".neukgu", "config.json")?,
            &serde_json::to_string_pretty(self)?,
            WriteMode::Atomic,
        )?)
    }

    pub fn system_prompt_context(&self) -> tera::Context {
        let mut result = tera::Context::new();
        result.insert("text_file_max_len", &self.text_file_max_len);
        result.insert("stdout_max_len", &self.stdout_max_len);
        result.insert("default_command_timeout", &self.default_command_timeout);
        result
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            model: String::from("sonnet"),
            sandbox_root: String::from("/tmp/neukgu-sandbox/"),
            llm_context_max_len: 204_800,
            text_file_max_len: 32_768,
            text_file_max_lines: 512,
            pdf_max_pages: 5,
            dir_max_entries: 256,
            stdout_max_len: 5120,
            default_command_timeout: 600,
            command_max_timeout: 3 * 3600,
        }
    }
}
