use std::path::PathBuf;

use anyhow::Result;
use globset::{Glob, GlobSetBuilder};
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::tools::{RenderError, ToolArgsRender, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct LsArgs {
    pub path: Option<PathBuf>,
    pub ignore: Option<Vec<String>>,
}

impl LsArgs {
    pub fn title(&self) -> String {
        let path = self.path.as_deref().unwrap_or_else(|| std::path::Path::new("."));
        format!("List the {} directory's contents", path.display())
    }
}

impl ToolArgsRender for LsArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.path.unwrap_or_else(|| PathBuf::from(".")).display().to_string();
        Ok((first, None))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct LsOutput {
    pub entries: Vec<String>,
}

impl ToolOutputRender for LsOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.entries.join("\n"))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LsError {
    #[error("Failed to read directory: {0}")]
    Io(#[from] std::io::Error),
}

pub const NAME: &str = "Ls";

pub struct LsTool {
    cwd: PathBuf,
}

impl LsTool {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }
}

impl Tool for LsTool {
    const NAME: &'static str = NAME;
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
                        "description": "The path to the directory to list (optional)"
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
        let dir_path = match args.path {
            Some(p) => p,
            None => self.cwd.clone(),
        };

        let mut builder = GlobSetBuilder::new();
        if let Some(ignores) = &args.ignore {
            for pattern in ignores {
                if let Ok(glob) = Glob::new(pattern) {
                    builder.add(glob);
                }
            }
        }
        let globset = builder.build().unwrap_or_default();

        let mut entries = Vec::new();

        let mut dir_entries = fs::read_dir(&dir_path).await?;

        while let Some(entry) = dir_entries.next_entry().await? {
            let path = entry.path();
            let file_name = match path.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => continue,
            };

            if globset.is_match(&file_name) || globset.is_match(&path) {
                continue;
            }

            let mut formatted_name = file_name;
            if path.is_dir() {
                formatted_name.push('/');
            }

            entries.push(formatted_name);
        }

        entries.sort();
        Ok(LsOutput { entries })
    }
}
