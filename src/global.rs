use chrono::Local;
use crate::{ContextJson, Error, NeukguId, load_json};
use ragit_fs::{
    WriteMode,
    basename,
    create_dir,
    exists,
    join,
    join3,
    read_dir,
    read_string,
    remove_file,
    write_string,
};
use serde::{Deserialize, Serialize};

pub struct Project {
    pub neukgu_id: NeukguId,
    pub working_dir: String,
    pub updated_at: i64,  // millis timestamp

    // error while loading this project
    pub error: Option<String>,
}

// serde-compatible type for `Project`
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProjectJson {
    pub working_dir: String,
    pub updated_at: i64,
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

    if !exists(&join(global_index_dir, "tmp-projects")?) {
        create_dir(&join(global_index_dir, "tmp-projects")?)?;
    }

    Ok(())
}

pub fn clean_global_index_dir(global_index_dir: &str) -> Result<(), Error> {
    let mut dangling_ids = vec![];

    for index in load_all_indexes(global_index_dir).iter() {
        if index.error.is_some() {
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

    Ok(())
}

pub fn update_global_index(
    working_dir: &str,
    global_index_dir: &str,
    neukgu_id: NeukguId,
) -> Result<(), Error> {
    let index_at = join3(global_index_dir, "indexes", &format!("{:016x}", neukgu_id.0))?;
    write_string(
        &index_at,
        &serde_json::to_string_pretty(&ProjectJson { working_dir: working_dir.to_string(), updated_at: Local::now().timestamp_millis() })?,
        WriteMode::Atomic,
    )?;
    Ok(())
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
                            error: None,
                        },
                        Err(e) => Project {
                            neukgu_id,
                            working_dir: String::from("????"),
                            updated_at: -1,
                            error: Some(format!("{e:?}")),
                        },
                    },
                    Err(e) => Project {
                        neukgu_id,
                        working_dir: String::from("????"),
                        updated_at: -1,
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
