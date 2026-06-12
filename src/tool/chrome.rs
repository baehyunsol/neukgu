use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum WebOrFile {
    Web(String),
    File(String),
}
