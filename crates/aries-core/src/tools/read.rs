use std::path::PathBuf;

use anyhow::Result;
use itertools::Itertools;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

use super::{RenderError, ToolArgsRender, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadArgs {
    pub file_path: PathBuf,
    pub offset: Option<usize>,
}

impl ReadArgs {
    pub fn location(&self) -> impl Into<PathBuf> {
        &self.file_path
    }
}

impl ToolArgsRender for ReadArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;

        let mut first = format!("{}", args.file_path.display());
        if let Some(offset) = args.offset {
            first.push_str(&format!(", offset = {offset}"));
        }

        Ok((first, None))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadOutput {
    pub content: String,
}

impl ToolOutputRender for ReadOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.content)
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
