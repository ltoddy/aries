use std::fmt::{self, Display};
use std::path::PathBuf;

use anyhow::Result;
use itertools::Itertools;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadArgs {
    pub file_path: PathBuf,
    pub offset: Option<usize>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadOutput {
    pub content: String,
}

impl Display for ReadOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.content)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ReadError {
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),
}

pub const NAME: &str = "Read";

pub struct ReadTool;

impl Tool for ReadTool {
    const NAME: &'static str = NAME;
    type Error = ReadError;
    type Args = ReadArgs;
    type Output = ReadOutput;

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

        Ok(ReadOutput { content })
    }
}
