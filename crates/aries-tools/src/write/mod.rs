mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use tokio::fs;

pub use self::args::WriteArgs;
pub use self::error::WriteError;
pub use self::output::WriteOutput;

pub const NAME: &str = "Write";

pub struct WriteTool;

impl WriteTool {
    pub fn new() -> Self {
        Self {}
    }
}

impl Tool for WriteTool {
    const NAME: &'static str = NAME;
    type Error = WriteError;
    type Args = WriteArgs;
    type Output = WriteOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("description.md").to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["file_path", "content"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if let Some(parent) = args.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&args.file_path, args.content).await?;

        Ok(WriteOutput { success: true })
    }
}
