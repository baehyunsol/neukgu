use chrono::Local;
use crate::{Context, ContextJson, Config, Error, NeukguId, init_log_dir, load_json};
use crate::chat::Config as ChatConfig;
use ragit_fs::{
    WriteMode,
    basename,
    create_dir,
    exists,
    join,
    join3,
    read_dir,
    read_string,
    remove_dir_all,
    remove_file,
    write_string,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub struct Project {
    pub neukgu_id: NeukguId,
    pub working_dir: String,
    pub updated_at: i64,  // millis timestamp
    pub is_in_global_index_dir: bool,

    // error while loading this project
    pub error: Option<String>,
}

// serde-compatible type for `Project`
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProjectJson {
    pub working_dir: String,
    pub updated_at: i64,

    // If it's created by "New project" button in the index tab,
    // 1. It doesn't have to display the full path.
    // 2. It's okay to delete the directory.
    pub is_in_global_index_dir: bool,
}

pub fn get_global_index_dir() -> Result<String, Error> {
    match std::env::var("NEUKGU_GLOBAL_INDEX") {
        Ok(path) => Ok(path),
        Err(_) => Ok(join(&std::env::var("HOME")?, ".herd-of-neukgu")?),
    }
}

pub fn init_global_index_dir(global_index_dir: &str) -> Result<(), Error> {
    if !exists(global_index_dir) {
        create_dir(global_index_dir)?;
    }

    if !exists(&join(global_index_dir, "indexes")?) {
        create_dir(&join(global_index_dir, "indexes")?)?;
    }

    if !exists(&join(global_index_dir, "projects")?) {
        create_dir(&join(global_index_dir, "projects")?)?;
    }

    if !exists(&join(global_index_dir, "chats")?) {
        let chats = join(global_index_dir, "chats")?;
        create_dir(&chats)?;

        // We need these directories to store images and logs.
        create_dir(&join(&chats, ".neukgu")?)?;
        create_dir(&join3(&chats, ".neukgu", "logs")?)?;
        create_dir(&join3(&chats, ".neukgu", "images")?)?;
        create_dir(&join3(&chats, ".neukgu", "interruptions")?)?;
        init_log_dir(&join3(&chats, ".neukgu", "logs")?)?;
    }

    if !exists(&join(global_index_dir, "chat-turns")?) {
        create_dir(&join(global_index_dir, "chat-turns")?)?;
    }

    if !exists(&join(global_index_dir, "cargo-targets")?) {
        create_dir(&join(global_index_dir, "cargo-targets")?)?;
    }

    if !exists(&join(global_index_dir, "config.json")?) {
        write_string(
            &join(global_index_dir, "config.json")?,
            &serde_json::to_string_pretty(&Config::default())?,
            WriteMode::AlwaysCreate,
        )?;
    }

    if !exists(&join(global_index_dir, "chat-config.json")?) {
        write_string(
            &join(global_index_dir, "chat-config.json")?,
            &serde_json::to_string_pretty(&ChatConfig::default())?,
            WriteMode::AlwaysCreate,
        )?;
    }

    Ok(())
}

pub fn clean_global_index_dir(global_index_dir: &str) -> Result<(), Error> {
    let mut dangling_ids = vec![];
    let mut valid_ids = HashSet::new();

    for index in load_all_indexes(global_index_dir).iter() {
        if index.error.is_some() {
            dangling_ids.push(index.neukgu_id);
            continue;
        }

        if !exists(&index.working_dir) {
            dangling_ids.push(index.neukgu_id);
            continue;
        }

        match load_json::<ContextJson>(&join3(&index.working_dir, ".neukgu", "context.json")?) {
            Ok(context) => {
                if context.neukgu_id != index.neukgu_id {
                    dangling_ids.push(index.neukgu_id);
                }

                else {
                    valid_ids.insert(format!("{:016x}", index.neukgu_id.0));
                }
            },
            Err(_) => {
                dangling_ids.push(index.neukgu_id);
            },
        }
    }

    for dangling_id in dangling_ids.iter() {
        let index_at = join3(global_index_dir, "indexes", &format!("{:016x}", dangling_id.0))?;
        remove_file(&index_at)?;
    }

    let cargo_targets_at = join(global_index_dir, "cargo-targets")?;

    for target in read_dir(&cargo_targets_at, false)? {
        let neukgu_id = basename(&target)?;

        if !valid_ids.contains(&neukgu_id) {
            remove_dir_all(&target)?;
        }
    }

    Ok(())
}

pub fn update_global_index(context: &Context) -> Result<(), Error> {
    let index_at = join3(&context.global_index_dir, "indexes", &format!("{:016x}", context.neukgu_id.0))?;
    let project = ProjectJson {
        working_dir: context.working_dir.to_string(),
        updated_at: Local::now().timestamp_millis(),
        is_in_global_index_dir: context.is_in_global_index_dir,
    };

    write_string(
        &index_at,
        &serde_json::to_string_pretty(&project)?,
        WriteMode::Atomic,
    )?;
    Ok(())
}

pub fn remove_global_index(global_index_dir: &str, id: NeukguId) -> Result<(), Error> {
    let index_at = join3(global_index_dir, "indexes", &format!("{:016x}", id.0))?;
    remove_file(&index_at)?;
    Ok(())
}

pub fn get_global_config(global_index_dir: &str) -> Result<Config, Error> {
    Ok(serde_json::from_str(&read_string(&join(global_index_dir, "config.json")?)?)?)
}

pub fn save_global_config(config: &Config, global_index_dir: &str) -> Result<(), Error> {
    Ok(write_string(
        &join(global_index_dir, "config.json")?,
        &serde_json::to_string_pretty(config)?,
        WriteMode::CreateOrTruncate,
    )?)
}

pub fn get_global_chat_config(global_index_dir: &str) -> Result<ChatConfig, Error> {
    Ok(serde_json::from_str(&read_string(&join(global_index_dir, "chat-config.json")?)?)?)
}

pub fn save_global_chat_config(config: &ChatConfig, global_index_dir: &str) -> Result<(), Error> {
    Ok(write_string(
        &join(global_index_dir, "chat-config.json")?,
        &serde_json::to_string_pretty(config)?,
        WriteMode::CreateOrTruncate,
    )?)
}

// It never fails, because `ui::gui::index` doesn't have a proper error handling method.
// Instead, it'll return an empty list if there's a critical error.
// and uses `.error` field if an individual index has an error.
pub fn load_all_indexes(global_index_dir: &str) -> Vec<Project> {
    let indexes_at = match join(global_index_dir, "indexes") {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error at `join({global_index_dir:?}, {:?})` in `load_all_indexes({global_index_dir:?})`: {e:?}", "indexes");
            return vec![];
        },
    };

    match read_dir(&indexes_at, false) {
        Ok(indexes) => {
            let mut result = vec![];

            for index in indexes.iter() {
                let basename = match basename(&index) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                let neukgu_id = match u64::from_str_radix(&basename, 16) {
                    Ok(id) => NeukguId(id),
                    Err(e) => {
                        eprintln!("error at `NeukguId::from_str({basename:?})` in `load_all_indexes({global_index_dir:?})`: {e:?}");
                        continue;
                    },
                };
                let index = match read_string(&index) {
                    Ok(p) => match serde_json::from_str::<ProjectJson>(&p) {
                        Ok(p) => Project {
                            neukgu_id,
                            working_dir: p.working_dir.to_string(),
                            updated_at: p.updated_at,
                            is_in_global_index_dir: p.is_in_global_index_dir,
                            error: None,
                        },
                        Err(e) => Project {
                            neukgu_id,
                            working_dir: String::from("????"),
                            updated_at: -1,
                            is_in_global_index_dir: false,
                            error: Some(format!("{e:?}")),
                        },
                    },
                    Err(e) => Project {
                        neukgu_id,
                        working_dir: String::from("????"),
                        updated_at: -1,
                        is_in_global_index_dir: false,
                        error: Some(format!("{e:?}")),
                    },
                };
                result.push(index);
            }

            result
        },
        Err(e) => {
            eprintln!("error at `read_dir({indexes_at:?})` in `load_all_indexes({global_index_dir:?})`: {e:?}");
            vec![]
        },
    }
}
