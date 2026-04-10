use std::path::PathBuf;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct EditOperation {
    pub old_string: String,
    pub new_string: String,
    pub replace_all: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MultiEditArgs {
    pub file_path: PathBuf,
    pub edits: Vec<EditOperation>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct MultiEditOutput {
    pub success: bool,
    pub message: String,
}

#[derive(thiserror::Error, Debug)]
pub enum MultiEditError {
    #[error("Failed to edit file: {0}")]
    EditError(String),
}

pub struct MultiEditTool;

impl Tool for MultiEditTool {
    const NAME: &'static str = "multiedit";
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
                                "old_string": {
                                    "type": "string",
                                    "description": "The text to replace"
                                },
                                "new_string": {
                                    "type": "string",
                                    "description": "The edited text to replace the old_string"
                                },
                                "replace_all": {
                                    "type": "boolean",
                                    "description": "Replace all occurrences of old_string"
                                }
                            },
                            "required": ["old_string", "new_string"]
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
            if edit.old_string == edit.new_string {
                return Err(MultiEditError::EditError("old_string and new_string cannot be identical".to_string()));
            }

            if edit.old_string.is_empty() {
                // If old_string is empty, we just append or initialize (simplified for MVP)
                content = edit.new_string;
                continue;
            }

            if !content.contains(&edit.old_string) {
                return Err(MultiEditError::EditError(format!(
                    "old_string not found in file (must match exactly including whitespace): {:?}",
                    edit.old_string
                )));
            }

            if edit.replace_all {
                content = content.replace(&edit.old_string, &edit.new_string);
            } else {
                content = content.replacen(&edit.old_string, &edit.new_string, 1);
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
