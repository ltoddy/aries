use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct EditOperation {
    pub old_text: String,
    pub new_text: String,
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MultiEditArgs {
    pub file_path: PathBuf,
    pub edits: Vec<EditOperation>,
}

impl MultiEditArgs {
    pub fn location(&self) -> impl Into<PathBuf> {
        &self.file_path
    }

    pub fn title(&self) -> String {
        format!("Edit file {} with {} changes", self.file_path.display(), self.edits.len())
    }
}

impl MultiEditArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;

        let first = format!("{}", args.file_path.display());

        let mut rest_lines = Vec::new();
        for edit in args.edits {
            let old_lines = edit.old_text.lines().map(|line| format!("- {}", line));
            let new_lines = edit.new_text.lines().map(|line| format!("+ {}", line));
            let diff = old_lines.chain(new_lines).collect::<Vec<_>>().join("\n");
            if !diff.is_empty() {
                rest_lines.push(diff);
            }
        }

        let rest = if rest_lines.is_empty() { None } else { Some(rest_lines.join("\n")) };
        Ok((first, rest))
    }
}
