use super::Path;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum WebOrFile {
    Web(String),
    File(Path),
}

impl WebOrFile {
    pub fn to_url(&self) -> String {
        match self {
            WebOrFile::Web(s) => s.to_string(),
            WebOrFile::File(p) => format!("file://{}", p.absolute),
        }
    }
}
