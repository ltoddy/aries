use std::{env, fs};

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct LsArgs {
    path: Option<String>,
    ignore: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct LsOutput {
    entries: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum LsError {
    #[error("Failed to read directory: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct LsTool;

impl Tool for LsTool {
    const NAME: &'static str = "ls";
    type Error = LsError;
    type Args = LsArgs;
    type Output = LsOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/ls.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The absolute path to the directory to list (optional)"
                    },
                    "ignore": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        },
                        "description": "Optional list of glob patterns to ignore"
                    }
                }
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let dir_path = args.path.unwrap_or_else(|| env::current_dir().unwrap().display().to_string());
        let mut entries = Vec::new();

        for entry in fs::read_dir(&dir_path)? {
            let entry = entry?;
            let path = entry.path();
            let mut file_name = path.file_name().unwrap().to_string_lossy().into_owned();

            if path.is_dir() {
                file_name.push('/');
            }

            // Note: ignore logic could be implemented here using globset or similar if
            // needed For MVP, we just return all entries
            entries.push(file_name);
        }

        entries.sort();
        Ok(LsOutput { entries })
    }
}
