use std::path::PathBuf;

use anyhow::Result;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::tools::{RenderError, ToolArgsRender, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteArgs {
    pub file_path: PathBuf,
    pub content: String,
}

impl WriteArgs {
    pub fn location(&self) -> impl Into<PathBuf> {
        &self.file_path
    }

    pub fn title(&self) -> String {
        format!("Write file {}", self.file_path.display())
    }
}

impl ToolArgsRender for WriteArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.file_path.display().to_string();
        let rest = Some(args.content);
        Ok((first, rest))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WriteOutput {
    pub success: bool,
}

impl ToolOutputRender for WriteOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(if output.success {
            "File written successfully".to_string()
        } else {
            "Failed to write file".to_string()
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum WriteError {
    #[error("Failed to write file: {0}")]
    IoError(#[from] std::io::Error),
}

pub const NAME: &str = "Write";

pub struct WriteTool;

impl Tool for WriteTool {
    const NAME: &'static str = NAME;
    type Error = WriteError;
    type Args = WriteArgs;
    type Output = WriteOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/write.txt").to_string(),
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
