use crate::{
    Agents,
    Error,
    EtcModels,
    PermissionConfig,
    Skill,
    SkillConfig,
    ToolKind,
    ToolPermissionKind,
    default_tool_permissions,
};
use crate::request::Config as RequestConfig;
use ragit_fs::{
    WriteMode,
    join3,
    read_string,
    write_string,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Config {
    pub agents: Agents,
    pub activated_tools: Vec<ToolKind>,
    pub skills: HashMap<String, SkillConfig>,
    pub sandbox_root: String,
    pub llm_context_max_len: u64,
    pub text_file_max_len: u64,
    pub text_file_max_lines: u64,
    pub pdf_max_pages: u64,
    pub dir_max_entries: u64,
    pub stdout_max_len: u64,
    pub default_command_timeout: u64,  // seconds
    pub user_response_timeout: u64,  // seconds
    pub tool_permissions: HashMap<ToolPermissionKind, PermissionConfig>,

    // A key is the name of the binary.
    // If an entry is missing, it's `PermissionConfig::Ask` by default.
    pub run_permissions: HashMap<String, PermissionConfig>,

    // If the agent doesn't write summary and keeps reading files,
    // the harness will force it to write a summary file.
    pub max_read_without_write: usize,

    // I'm worried if AI mistakens millisec and sec.
    pub command_max_timeout: u64,  // seconds

    pub etc_models: EtcModels,
}

impl Config {
    pub fn load(working_dir: &str) -> Result<Self, Error> {
        let s = read_string(&join3(working_dir, ".neukgu", "config.json")?)?;
        Ok(serde_json::from_str(&s)?)
    }

    pub fn store(&self, working_dir: &str) -> Result<(), Error> {
        Ok(write_string(
            &join3(working_dir, ".neukgu", "config.json")?,
            &serde_json::to_string_pretty(self)?,
            WriteMode::Atomic,
        )?)
    }

    pub fn request_config(&self) -> RequestConfig {
        RequestConfig {
            etc_models: self.etc_models.clone(),
            ..RequestConfig::default()
        }
    }

    pub fn add_skill(&mut self, skill: Skill) {
        self.skills.insert(skill.name.to_string(), skill.to_config(true));
    }

    pub fn remove_skill(&mut self, skill: &str) {
        self.skills.remove(skill);
    }
}

impl Default for Config {
    fn default() -> Config {
        Config {
            agents: Agents::default(),
            activated_tools: ToolKind::all(),
            skills: HashMap::new(),
            sandbox_root: String::from("/tmp/neukgu-sandbox/"),
            llm_context_max_len: 262_144,
            text_file_max_len: 32_768,
            text_file_max_lines: 512,
            pdf_max_pages: 5,
            dir_max_entries: 512,
            stdout_max_len: 5120,
            default_command_timeout: 600,
            user_response_timeout: 300,
            tool_permissions: default_tool_permissions(),
            run_permissions: HashMap::new(),
            max_read_without_write: 6,
            command_max_timeout: 3 * 3600,
            etc_models: EtcModels::default(),
        }
    }
}
