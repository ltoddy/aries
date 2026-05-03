use std::path::PathBuf;

use anyhow::Result;
use itertools::Itertools;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadFileArgs {
    pub file_path: PathBuf,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadFileOutput {
    pub content: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ReadFileError {
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),
}

pub const NAME: &str = "read_file";

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    const NAME: &'static str = NAME;
    type Error = ReadFileError;
    type Args = ReadFileArgs;
    type Output = ReadFileOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/read.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "file_path": {
                        "type": "string",
                        "description": "The path to the file to read"
                    },
                    "offset": {
                        "type": "number",
                        "description": "The line number to start reading from (1-indexed)"
                    }
                },
                "required": ["file_path"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut content = fs::read_to_string(&args.file_path).await?;

        let offset = args.offset.map(|offset| offset.saturating_sub(1));
        if let Some(offset) = offset {
            content = content.lines().skip(offset).join("\n");
        }

        Ok(ReadFileOutput { content })
    }
}
