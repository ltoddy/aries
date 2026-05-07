use std::fmt::{self, Display};
use std::path::PathBuf;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteFileArgs {
    pub file_path: PathBuf,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteFileOutput {
    pub success: bool,
}

impl Display for WriteFileOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.success {
            write!(f, "File written successfully")
        } else {
            write!(f, "Failed to write file")
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WriteFileError {
    #[error("Failed to write file: {0}")]
    IoError(#[from] std::io::Error),
}

pub const NAME: &str = "write_file";

pub struct WriteFileTool;

impl Tool for WriteFileTool {
    const NAME: &'static str = NAME;
    type Error = WriteFileError;
    type Args = WriteFileArgs;
    type Output = WriteFileOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/write.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The absolute path to the file to write"
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

        Ok(WriteFileOutput { success: true })
    }
}
