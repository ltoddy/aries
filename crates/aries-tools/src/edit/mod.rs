mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use tokio::fs;

pub use self::args::EditArgs;
pub use self::error::EditError;
pub use self::output::EditOutput;

pub const NAME: &str = "Edit";

pub struct EditTool;

impl EditTool {
    pub fn new() -> Self {
        Self {}
    }
}

impl Tool for EditTool {
    const NAME: &'static str = NAME;
    type Error = EditError;
    type Args = EditArgs;
    type Output = EditOutput;

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
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if !args.file_path.exists() {
            return Err(EditError::EditError(
                "File does not exist. Use write_file to create new files.".to_owned(),
            ));
        }

        let content = fs::read_to_string(&args.file_path)
            .await
            .map_err(|e| EditError::EditError(format!("Failed to read file: {}", e)))?;

        if args.old_text == args.new_text {
            return Err(EditError::EditError(
                "oldString and newString cannot be identical".to_owned(),
            ));
        }

        if !content.contains(&args.old_text) {
            return Err(EditError::EditError("oldString not found in content".to_owned()));
        }

        let occurrences = content.matches(&args.old_text).count();
        if occurrences > 1 && !args.replace_all {
            return Err(EditError::EditError("Found multiple matches for oldString. Provide more surrounding lines in oldString to identify the correct match or use replaceAll.".to_owned()));
        }

        let new_content = if args.replace_all {
            content.replace(&args.old_text, &args.new_text)
        } else {
            content.replacen(&args.old_text, &args.new_text, 1)
        };

        fs::write(&args.file_path, new_content)
            .await
            .map_err(|e| EditError::EditError(format!("Failed to write file: {}", e)))?;

        Ok(EditOutput { success: true, message: "Edit applied successfully".to_owned() })
    }
}
