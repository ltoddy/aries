use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GlobArgs {
    pattern: String,
}

#[derive(Serialize)]
pub struct GlobOutput {
    files: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum GlobError {
    #[error("Glob pattern error: {0}")]
    PatternError(#[from] glob::PatternError),
}

pub struct GlobTool;

impl Tool for GlobTool {
    const NAME: &'static str = "glob";
    type Error = GlobError;
    type Args = GlobArgs;
    type Output = GlobOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/glob.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The glob pattern to match against (e.g., src/**/*.rs)"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut files = Vec::new();

        for entry in glob::glob(&args.pattern)? {
            match entry {
                Ok(path) => files.push(path.display().to_string()),
                Err(e) => eprintln!("Glob warning: {:?}", e),
            }
        }

        Ok(GlobOutput { files })
    }
}
