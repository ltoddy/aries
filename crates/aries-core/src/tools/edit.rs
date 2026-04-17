use std::path::PathBuf;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct EditArgs {
    pub file_path: PathBuf,
    pub old_string: String,
    pub new_string: String,
    #[serde(default)]
    pub replace_all: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EditOutput {
    pub success: bool,
    pub message: String,
}

#[derive(thiserror::Error, Debug)]
pub enum EditError {
    #[error("Failed to edit file: {0}")]
    EditError(String),
}

pub const NAME: &str = "edit";

pub struct EditTool;

impl Tool for EditTool {
    const NAME: &'static str = NAME;
    type Error = EditError;
    type Args = EditArgs;
    type Output = EditOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/edit.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file to modify"
                    },
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
                "required": ["file_path", "old_string", "new_string"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if !args.file_path.exists() {
            return Err(EditError::EditError(
                "File does not exist. Use write_file to create new files.".to_string(),
            ));
        }

        let content = fs::read_to_string(&args.file_path)
            .await
            .map_err(|e| EditError::EditError(format!("Failed to read file: {}", e)))?;

        if args.old_string == args.new_string {
            return Err(EditError::EditError(
                "oldString and newString cannot be identical".to_string(),
            ));
        }

        if !content.contains(&args.old_string) {
            return Err(EditError::EditError("oldString not found in content".to_string()));
        }

        let occurrences = content.matches(&args.old_string).count();
        if occurrences > 1 && !args.replace_all {
            return Err(EditError::EditError("Found multiple matches for oldString. Provide more surrounding lines in oldString to identify the correct match or use replaceAll.".to_string()));
        }

        let new_content = if args.replace_all {
            content.replace(&args.old_string, &args.new_string)
        } else {
            content.replacen(&args.old_string, &args.new_string, 1)
        };

        fs::write(&args.file_path, new_content)
            .await
            .map_err(|e| EditError::EditError(format!("Failed to write file: {}", e)))?;

        Ok(EditOutput { success: true, message: "Edit applied successfully".to_string() })
    }
}
