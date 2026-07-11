mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use tokio::fs;

pub use self::args::{EditOperation, MultiEditArgs};
pub use self::error::MultiEditError;
pub use self::output::MultiEditOutput;

pub const NAME: &str = "MultiEdit";

pub struct MultiEditTool;

impl Default for MultiEditTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiEditTool {
    pub fn new() -> Self {
        Self {}
    }
}

impl Tool for MultiEditTool {
    const NAME: &'static str = NAME;
    type Error = MultiEditError;
    type Args = MultiEditArgs;
    type Output = MultiEditOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("description.md").to_owned(),
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
                    "old_text and new_text cannot be identical".to_owned(),
                ));
            }

            if edit.old_text.is_empty() {
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

        Ok(MultiEditOutput { success: true, message: "Edits applied successfully".to_owned() })
    }
}
