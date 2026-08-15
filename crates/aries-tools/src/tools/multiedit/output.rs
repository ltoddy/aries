use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tools::diff;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MultiEditOutput {
    #[serde(rename = "type")]
    pub kind: WriteKind,
    pub file_path: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structured_patch: Vec<diff::Hunk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,
    pub additions: usize,
    pub deletions: usize,
}

impl MultiEditOutput {
    pub fn for_create(file_path: impl AsRef<Path>, additions: usize) -> Self {
        let file_path = file_path.as_ref().to_path_buf();

        Self {
            kind: WriteKind::Create,
            file_path,
            structured_patch: vec![],
            original_content: None,
            additions,
            deletions: 0,
        }
    }

    pub fn for_update(
        file_path: impl AsRef<Path>,
        original_content: impl Into<String>,
        structured_patch: Vec<diff::Hunk>,
        additions: usize,
        deletions: usize,
    ) -> Self {
        let file_path = file_path.as_ref().to_path_buf();
        let original_content = original_content.into();

        Self {
            kind: WriteKind::Update,
            file_path,
            structured_patch,
            original_content: Some(original_content),
            additions,
            deletions,
        }
    }
}

impl MultiEditOutput {
    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output = serde_json::from_value::<MultiEditOutput>(raw.clone())?;
        let path = output.file_path.display();
        Ok(match output.kind {
            WriteKind::Create => format!("File created successfully at: {}", path),
            WriteKind::Update => format!("The file {} has been updated successfully.", path),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WriteKind {
    Create,
    Update,
}
