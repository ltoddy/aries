use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolArgsRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct BashArgs {
    pub command: String,
}

impl BashArgs {
    pub fn title(&self) -> String {
        let command = self.command.trim();
        if command.is_empty() {
            "Run a shell command".to_owned()
        } else {
            format!("Run shell command: {command}")
        }
    }
}

impl ToolArgsRender for BashArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.command;
        Ok((first, None))
    }
}
