use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tools::diff;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EditOutput {
    pub file_path: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structured_patch: Vec<diff::Hunk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,
    pub additions: usize,
    pub deletions: usize,
}

impl EditOutput {
    pub fn new(
        file_path: impl AsRef<Path>,
        original_content: impl Into<String>,
        structured_patch: Vec<diff::Hunk>,
        additions: usize,
        deletions: usize,
    ) -> Self {
        let file_path = file_path.as_ref();
        let original_content = original_content.into();

        Self {
            file_path: file_path.to_owned(),
            structured_patch,
            original_content: Some(original_content),
            additions,
            deletions,
        }
    }
}

impl EditOutput {
    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!("The file {} has been updated successfully.", output.file_path.display()))
    }
}
