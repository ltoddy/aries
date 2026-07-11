use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolArgsRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct EditArgs {
    pub file_path: PathBuf,
    pub old_text: String,
    pub new_text: String,
    #[serde(default)]
    pub replace_all: bool,
}

impl EditArgs {
    pub fn location(&self) -> impl Into<PathBuf> {
        &self.file_path
    }

    pub fn title(&self) -> String {
        format!("Edit file {}", self.file_path.display())
    }
}

impl ToolArgsRender for EditArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = format!("{}", args.file_path.display());
        if args.replace_all {
            first.push_str(" replace_all = true");
        }

        let old_lines = args.old_text.lines().map(|line| format!("- {}", line));
        let new_lines = args.new_text.lines().map(|line| format!("+ {}", line));
        let diff = old_lines.chain(new_lines).collect::<Vec<_>>().join("\n");
        let rest = if diff.is_empty() { None } else { Some(diff) };

        Ok((first, rest))
    }
}
