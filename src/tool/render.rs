use super::{Path, normalize_path};
use crate::Error;
use ragit_fs::{current_dir, join};

#[derive(Clone, Debug)]
pub enum WebOrFile {
    Web(String),
    File(Path),
}

impl WebOrFile {
    pub fn to_url(&self) -> Result<String, Error> {
        match self {
            WebOrFile::Web(s) => Ok(s.to_string()),
            WebOrFile::File(p) => {
                // read_permission is already checked, so it's safe to unwrap this
                let p = normalize_path(p).unwrap().join("/");
                let p = join(&current_dir()?, &p)?;
                Ok(format!("file://{p}"))
            },
        }
    }
}

impl From<&Path> for WebOrFile {
    fn from(path: &Path) -> WebOrFile {
        let joined_path = path.join("/");

        // What a naive algorithm hahaha
        if joined_path.starts_with("http:") || joined_path.starts_with("https:") {
            WebOrFile::Web(joined_path)
        } else {
            WebOrFile::File(path.clone())
        }
    }
}
