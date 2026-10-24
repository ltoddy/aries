mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::fs;

pub use self::args::{EditOperation, MultiEditArgs};
pub use self::error::{MultiEditError, MultiEditIoOperation};
pub use self::output::{MultiEditOutput, WriteKind};
use crate::tools::diff;

pub const NAME: &str = "MultiEdit";

pub struct MultiEditTool {
    ctx: crate::context::ToolContext,
}

impl MultiEditTool {
    pub fn new(ctx: crate::context::ToolContext) -> Self {
        Self { ctx }
    }
}

impl Tool for MultiEditTool {
    const NAME: &'static str = NAME;
    type Args = MultiEditArgs;
    type Output = MultiEditOutput;
    type Error = MultiEditError;

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
                "edits": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
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
                        "required": ["old_text", "new_text"]
                    }
                }
            },
            "required": ["file_path", "edits"]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let cwd = self
            .ctx
            .worktree()
            .map_or_else(|| self.ctx.cwd(), |worktree| worktree.path().to_owned());
        let file_path =
            if args.file_path.is_relative() { cwd.join(&args.file_path) } else { args.file_path };

        let original_content = if file_path.exists() {
            self.ctx.guard_write(&file_path).await?;
            let content = fs::read_to_string(&file_path).await.map_err(|err| {
                MultiEditError::io(MultiEditIoOperation::ReadFile, &file_path, err)
            })?;
            Some(content)
        } else {
            None
        };

        let mut content = original_content.clone().unwrap_or_default();

        for edit in args.edits {
            if edit.old_text == edit.new_text {
                return Err(MultiEditError::identical_text());
            }

            if edit.old_text.is_empty() {
                if !content.is_empty() {
                    return Err(MultiEditError::empty_old_text());
                }
                content = edit.new_text;
                continue;
            }

            if !content.contains(&edit.old_text) {
                return Err(MultiEditError::old_text_not_found(edit.old_text));
            }

            if edit.replace_all {
                content = content.replace(&edit.old_text, &edit.new_text);
            } else {
                content = content.replacen(&edit.old_text, &edit.new_text, 1);
            }
        }

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await.map_err(|err| {
                MultiEditError::io(MultiEditIoOperation::CreateParentDirectory, &file_path, err)
            })?;
        }
        fs::write(&file_path, &content)
            .await
            .map_err(|err| MultiEditError::io(MultiEditIoOperation::WriteFile, &file_path, err))?;
        self.ctx.on_file_written(&file_path, &content).await;

        match original_content {
            Some(original_content) => {
                let (hunks, additions, deletions) = diff::diff(&original_content, &content);
                Ok(MultiEditOutput::for_update(
                    file_path,
                    original_content,
                    hunks,
                    additions,
                    deletions,
                ))
            },
            None => {
                let additions = content.lines().count();
                Ok(MultiEditOutput::for_create(file_path, additions))
            },
        }
    }
}
