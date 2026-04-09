use std::path::PathBuf;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Deserialize)]
pub struct WriteFileArgs {
    pub file_path: PathBuf,
    pub content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WriteFileOutput {
    pub success: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum WriteFileError {
    #[error("Failed to write file: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct WriteFileTool;

impl Tool for WriteFileTool {
    const NAME: &'static str = "write_file";
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
