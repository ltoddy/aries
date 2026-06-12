use std::path::PathBuf;

use anyhow::Result;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::tools::{RenderError, ToolArgsRender, ToolOutputRender};

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
}

impl ToolArgsRender for MultiEditArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
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

#[derive(Debug, Deserialize, Serialize)]
pub struct MultiEditOutput {
    pub success: bool,
    pub message: String,
}

impl ToolOutputRender for MultiEditOutput {
    fn render_output(raw: &str) -> std::result::Result<String, RenderError> {
        let output = serde_json::from_str::<MultiEditOutput>(raw)?;
        Ok(output.message)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MultiEditError {
    #[error("Failed to edit file: {0}")]
    EditError(String),
}

pub const NAME: &str = "MultiEdit";

pub struct MultiEditTool;

impl Tool for MultiEditTool {
    const NAME: &'static str = NAME;
    type Error = MultiEditError;
    type Args = MultiEditArgs;
    type Output = MultiEditOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/multiedit.txt").to_string(),
            parameters: serde_json::json!({
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
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut content = if args.file_path.exists() {
            fs::read_to_string(&args.file_path)
                .await
                .map_err(|e| MultiEditError::EditError(format!("Failed to read file: {}", e)))?
        } else {
            String::new()
        };

        for edit in args.edits {
            if edit.old_text == edit.new_text {
                return Err(MultiEditError::EditError(
                    "old_text and new_text cannot be identical".to_string(),
                ));
            }

            if edit.old_text.is_empty() {
                // If old_text is empty, we just append or initialize (simplified for MVP)
                content = edit.new_text;
                continue;
            }

            if !content.contains(&edit.old_text) {
                return Err(MultiEditError::EditError(format!(
                    "old_text not found in file (must match exactly including whitespace): {:?}",
                    edit.old_text
                )));
            }

            if edit.replace_all {
                content = content.replace(&edit.old_text, &edit.new_text);
            } else {
                content = content.replacen(&edit.old_text, &edit.new_text, 1);
            }
        }

        if let Some(parent) = args.file_path.parent() {
            let _ = fs::create_dir_all(parent).await;
        }

        fs::write(&args.file_path, content)
            .await
            .map_err(|e| MultiEditError::EditError(format!("Failed to write file: {}", e)))?;

        Ok(MultiEditOutput { success: true, message: "Edits applied successfully".to_string() })
    }
}
