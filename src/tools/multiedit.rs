use std::path::Path;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Deserialize)]
pub struct EditOperation {
    #[serde(rename = "oldString")]
    old_string: String,
    #[serde(rename = "newString")]
    new_string: String,
    #[serde(rename = "replaceAll", default)]
    replace_all: bool,
}

#[derive(Deserialize)]
pub struct MultiEditArgs {
    file_path: String,
    edits: Vec<EditOperation>,
}

#[derive(Serialize)]
pub struct MultiEditOutput {
    success: bool,
    message: String,
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
                        "description": "The absolute path to the file to modify"
                    },
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "oldString": {
                                    "type": "string",
                                    "description": "The text to replace"
                                },
                                "newString": {
                                    "type": "string",
                                    "description": "The edited text to replace the oldString"
                                },
                                "replaceAll": {
                                    "type": "boolean",
                                    "description": "Replace all occurrences of oldString"
                                }
                            },
                            "required": ["oldString", "newString"]
                        }
                    }
                },
                "required": ["file_path", "edits"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut content = if Path::new(&args.file_path).exists() {
            fs::read_to_string(&args.file_path)
                .await
                .map_err(|e| MultiEditError::EditError(format!("Failed to read file: {}", e)))?
        } else {
            String::new()
        };

        for edit in args.edits {
            if edit.old_string == edit.new_string {
                return Err(MultiEditError::EditError("oldString and newString cannot be identical".to_string()));
            }

            if edit.old_string.is_empty() {
                // If oldString is empty, we just append or initialize (simplified for MVP)
                content = edit.new_string;
                continue;
            }

            if !content.contains(&edit.old_string) {
                return Err(MultiEditError::EditError(format!(
                    "oldString not found in file (must match exactly including whitespace): {:?}",
                    edit.old_string
                )));
            }

            if edit.replace_all {
                content = content.replace(&edit.old_string, &edit.new_string);
            } else {
                content = content.replacen(&edit.old_string, &edit.new_string, 1);
            }
        }

        if let Some(parent) = Path::new(&args.file_path).parent() {
            let _ = fs::create_dir_all(parent).await;
        }

        fs::write(&args.file_path, content)
            .await
            .map_err(|e| MultiEditError::EditError(format!("Failed to write file: {}", e)))?;

        Ok(MultiEditOutput { success: true, message: "Edits applied successfully".to_string() })
    }
}
