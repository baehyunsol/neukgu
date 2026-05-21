use super::{Path, ToolCallError, check_read_permission, normalize_path};
use ragit_fs::{exists, is_dir, join};
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
) -> Result<(String, String), ToolCallError> {
    let joined_path = match normalize_path(path) {
        Some(path) if path.is_empty() => String::from("."),
        Some(path) => path.join("/"),
        None => path.join("/"),
    };

    if !check_read_permission(path) {
        return Err(ToolCallError::NoPermissionToWrite { path: joined_path });
    }

    // If `join` fails, `check_read_permission` should have caught that!
    let real_path = join(working_dir, &joined_path).unwrap();

    match (mode, exists(&real_path)) {
        (Some(WriteMode::Truncate | WriteMode::Append), _) if is_dir(&real_path) => {
            return Err(ToolCallError::CannotWriteToDirectory { path: joined_path, exists: exists(&real_path) });
        },
        (Some(WriteMode::Create), false) |
        (Some(WriteMode::Truncate), true) |
        (Some(WriteMode::Append), true) => {},
        (Some(mode), exists) => {
            return Err(ToolCallError::WriteModeError {
                path: joined_path,
                mode,
                exists,
            });
        },
        _ => {},
    }

    if joined_path == "." || joined_path.ends_with("/") || is_dir(&real_path) {
        return Err(ToolCallError::CannotWriteToDirectory { path: joined_path, exists: exists(&real_path) });
    }

    Ok((joined_path, real_path))
}
