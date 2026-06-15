use super::{Path, ToolCallError, normalize_path};
use ragit_fs::{create_dir_all, exists, is_dir, parent};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Deserialize, PartialEq, Serialize)]
pub enum WriteMode {
    Create,
    Truncate,
    Append,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum DumpOrRedirect {
    Dump(String),
    Redirect(Path),
}

impl From<WriteMode> for ragit_fs::WriteMode {
    fn from(m: WriteMode) -> ragit_fs::WriteMode {
        match m {
            WriteMode::Create => ragit_fs::WriteMode::AlwaysCreate,
            WriteMode::Truncate => ragit_fs::WriteMode::CreateOrTruncate,
            WriteMode::Append => ragit_fs::WriteMode::AlwaysAppend,
        }
    }
}

pub fn check_write_path(
    path: &str,
    working_dir: &str,

    // If it's None, it treats `mode` like `ragit_fs::WriteMode::CreateOrTruncate`
    mode: Option<WriteMode>,
) -> Result<Path, ToolCallError> {
    let path_ends_with_slash = path.ends_with("/");
    let path = match normalize_path(path, working_dir) {
        Some(path) => path,
        None => return Err(ToolCallError::InvalidPath(path.to_string())),
    };

    if path.is_index_dir() {
        return Err(ToolCallError::CannotWriteToIndexDir);
    }

    match (mode, exists(&path.absolute)) {
        (Some(WriteMode::Truncate | WriteMode::Append), _) if is_dir(&path.absolute) => {
            return Err(ToolCallError::CannotWriteToDirectory { path: path.clone(), exists: exists(&path.absolute) });
        },
        (Some(WriteMode::Create), false) |
        (Some(WriteMode::Truncate), true) |
        (Some(WriteMode::Append), true) => {},
        (Some(mode), exists) => {
            return Err(ToolCallError::WriteModeError {
                path,
                mode,
                exists,
            });
        },
        _ => {},
    }

    if path.relative.as_ref().unwrap_or(&String::new()) == "." || path_ends_with_slash || is_dir(&path.absolute) {
        return Err(ToolCallError::CannotWriteToDirectory { path: path.clone(), exists: exists(&path.absolute) });
    }

    let parent_path = match parent(&path.absolute) {
        Ok(p) => p,
        Err(_) => return Err(ToolCallError::InvalidPath(path.to_string())),
    };

    if !exists(&parent_path) {
        if let Err(e) = create_dir_all(&parent_path) {
            return Err(ToolCallError::CannotCreateParentDirectory { parent: parent(&path.to_string()).unwrap(), path, error: Some(format!("{e:?}")) });
        }
    }

    else if !is_dir(&parent_path) {
        return Err(ToolCallError::CannotCreateParentDirectory { parent: parent(&path.to_string()).unwrap(), path, error: Some(format!("`{parent_path}` already exists and is not a directory.")) });
    }

    Ok(path)
}
