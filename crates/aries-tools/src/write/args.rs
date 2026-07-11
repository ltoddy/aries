use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteArgs {
    pub file_path: PathBuf,
    pub content: String,
}

impl WriteArgs {
    pub fn location(&self) -> impl Into<PathBuf> {
        &self.file_path
    }

    pub fn title(&self) -> String {
        format!("Write file {}", self.file_path.display())
    }
}
