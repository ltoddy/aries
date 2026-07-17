use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use similar::{DiffOp, TextDiff};

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteOutput {
    #[serde(rename = "type")]
    pub kind: WriteKind,
    pub file_path: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub structured_patch: Vec<Hunk>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub original_content: Option<String>,
    pub additions: usize,
    pub deletions: usize,
}

impl WriteOutput {
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
        structured_patch: Vec<Hunk>,
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

impl WriteOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Hunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    // 带有 patch 的前缀
    pub lines: Vec<String>,
}

impl Hunk {
    pub fn new(
        old_start: usize,
        old_lines: usize,
        new_start: usize,
        new_lines: usize,
        lines: Vec<String>,
    ) -> Self {
        Self { old_start, old_lines, new_start, new_lines, lines }
    }

    pub fn from_diff(op: &DiffOp, diff: &TextDiff<str>) -> Self {
        let old_start = op.old_range().start;
        let old_lines = op.old_range().len();
        let new_start = op.new_range().start;
        let new_lines = op.new_range().len();

        let mut lines = Vec::new();
        for idx in op.old_range() {
            if let Some(line) = diff.old_slice(idx) {
                lines.push(format!("-{}", line.trim_end_matches('\n')));
            }
        }
        for idx in op.new_range() {
            if let Some(line) = diff.new_slice(idx) {
                lines.push(format!("+{}", line.trim_end_matches('\n')));
            }
        }

        Self::new(old_start, old_lines, new_start, new_lines, lines)
    }
}
