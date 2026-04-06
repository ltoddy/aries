use std::path::PathBuf;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Deserialize)]
pub struct ReadFileArgs {
    file_path: PathBuf,
    offset: Option<usize>,
}

#[derive(Serialize, Deserialize)]
pub struct ReadFileOutput {
    pub content: String,
}

#[derive(thiserror::Error, Debug)]
pub enum ReadFileError {
    #[error("Failed to read file: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct ReadFileTool;

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";
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
                        "description": "The absolute path to the file to read"
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
        let content = fs::read_to_string(&args.file_path).await?;

        let lines = content.lines().enumerate();
        let offset = args.offset.unwrap_or(1).saturating_sub(1);

        let limited_content: String = lines
            .skip(offset)
            .take(2000) // Default limit as per prompt
            .map(|(i, line)| {
                let truncated_line = if line.len() > 2000 {
                    format!("{}... (truncated)", &line[..2000])
                } else {
                    line.to_string()
                };
                format!("{}: {}\n", i + 1, truncated_line)
            })
            .collect();

        Ok(ReadFileOutput { content: limited_content })
    }
}
