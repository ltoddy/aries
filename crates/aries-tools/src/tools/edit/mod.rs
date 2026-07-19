mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use rig_core::tool::Tool;
use serde_json::Value;
use tokio::fs;

pub use self::args::EditArgs;
pub use self::error::EditError;
pub use self::output::EditOutput;
use crate::context::ToolContext;
use crate::tools::diff;

pub const NAME: &str = "Edit";

pub struct EditTool {
    cwd: PathBuf,
    ctx: ToolContext,
}

impl EditTool {
    pub fn new(cwd: impl AsRef<Path>, ctx: ToolContext) -> Self {
        let cwd = cwd.as_ref().to_path_buf();

        Self { cwd, ctx }
    }
}

impl Tool for EditTool {
    const NAME: &'static str = NAME;
    type Error = EditError;
    type Args = EditArgs;
    type Output = EditOutput;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to modify"
                },
                "old_text": {
                    "type": "string",
                    "description": "The text to replace"
                },
                "new_text": {
                    "type": "string",
                    "description": "The edited text to replace the old_text"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences of old_text"
                }
            },
            "required": ["file_path", "old_text", "new_text"]
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let file_path = if args.file_path.is_relative() {
            self.cwd.join(&args.file_path)
        } else {
            args.file_path
        };

        if args.old_text == args.new_text {
            return Err(EditError::identical_text());
        }
        if !file_path.exists() {
            return Err(EditError::file_not_found(file_path));
        }

        self.ctx.guard_write(&file_path).await?;

        let content = fs::read_to_string(&file_path).await?;
        if !content.contains(&args.old_text) {
            return Err(EditError::old_text_not_found());
        }

        let occurrences = content.matches(&args.old_text).count();
        if occurrences > 1 && !args.replace_all {
            return Err(EditError::multiple_matches(occurrences));
        }

        let new_content = if args.replace_all {
            content.replace(&args.old_text, &args.new_text)
        } else {
            content.replacen(&args.old_text, &args.new_text, 1)
        };

        let _ = self.ctx.file_checkpoint.push(&file_path, &content).await;
        fs::write(&file_path, &new_content).await?;

        self.ctx.on_file_written(&file_path, &new_content).await;

        let (hunks, additions, deletions) = diff::diff(&content, &new_content);

        Ok(EditOutput::new(file_path, content, hunks, additions, deletions))
    }
}
