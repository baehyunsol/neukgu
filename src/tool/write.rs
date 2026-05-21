use super::{Path, ToolCallError, check_read_permission, normalize_path};
use crate::Error;
use ragit_fs::{create_dir_all, exists, is_dir, is_symlink, join, parent};
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
    path: &Path,
    working_dir: &str,

    // If it's None, it treats `mode` like `ragit_fs::WriteMode::CreateOrTruncate`
    mode: Option<WriteMode>,
) -> Result<Result<(String, String), ToolCallError>, Error> {
    let path_ends_with_slash = path.last().map(|p| p.is_empty()) == Some(true);
    let joined_path = match normalize_path(path) {
        Some(path) if path.is_empty() => String::from("."),
        Some(path) => path.join("/"),
        None => path.join("/"),
    };

    if !check_read_permission(path) {
        return Ok(Err(ToolCallError::NoPermissionToWrite { path: joined_path }));
    }

    let real_path = join(working_dir, &joined_path)?;

    match (mode, exists(&real_path)) {
        (Some(WriteMode::Truncate | WriteMode::Append), _) if is_dir(&real_path) => {
            return Ok(Err(ToolCallError::CannotWriteToDirectory { path: joined_path, exists: exists(&real_path) }));
        },
        (Some(WriteMode::Create), false) |
        (Some(WriteMode::Truncate), true) |
        (Some(WriteMode::Append), true) => {},
        (Some(mode), exists) => {
            return Ok(Err(ToolCallError::WriteModeError {
                path: joined_path,
                mode,
                exists,
            }));
        },
        _ => {},
    }

    if joined_path == "." || path_ends_with_slash || is_dir(&real_path) {
        return Ok(Err(ToolCallError::CannotWriteToDirectory { path: joined_path, exists: exists(&real_path) }));
    }

    let parent_path = parent(&real_path)?;

    if is_symlink(&parent_path) {
        return Ok(Err(ToolCallError::CannotCreateParentDirectory { parent: parent(&joined_path)?, file: joined_path }));
    }

    else if !exists(&parent_path) {
        create_dir_all(&parent_path)?;
    }

    else if !is_dir(&parent_path) {
        return Ok(Err(ToolCallError::CannotCreateParentDirectory { parent: parent(&joined_path)?, file: joined_path }));
    }

    Ok(Ok((joined_path, real_path)))
}
