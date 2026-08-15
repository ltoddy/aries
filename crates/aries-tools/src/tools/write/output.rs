use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WriteOutput {
    pub file_path: PathBuf,
    pub additions: usize,
}

impl WriteOutput {
    pub fn new(file_path: impl AsRef<Path>, additions: usize) -> Self {
        Self { file_path: file_path.as_ref().to_path_buf(), additions }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!("File created successfully at: {}", output.file_path.display()))
    }
}
