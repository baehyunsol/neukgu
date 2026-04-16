use super::{Path, check_read_permission};
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

// There used to be a strict restriction, but I decided to give more permission to AIs.
pub fn check_write_permission(path: &Path) -> bool {
    check_read_permission(path)
}
