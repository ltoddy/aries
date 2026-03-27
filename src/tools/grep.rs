use anyhow::Result;
use regex::Regex;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Deserialize)]
pub struct GrepArgs {
    pattern: String,
    include: Option<String>,
}

#[derive(Serialize)]
pub struct GrepOutput {
    matches: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum GrepError {
    #[error("Regex error: {0}")]
    RegexError(#[from] regex::Error),
    #[error("Glob error: {0}")]
    GlobError(#[from] glob::PatternError),
}

pub struct GrepTool;

impl Tool for GrepTool {
    const NAME: &'static str = "grep";
    type Error = GrepError;
    type Args = GrepArgs;
    type Output = GrepOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/grep.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The regular expression to search for"
                    },
                    "include": {
                        "type": "string",
                        "description": "Optional glob pattern to filter files (e.g., src/**/*.rs)"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let re = Regex::new(&args.pattern)?;
        let mut matches = Vec::new();

        let glob_pattern = args.include.unwrap_or_else(|| "**/*".to_string());

        for path in glob::glob(&glob_pattern)?.flatten() {
            if path.is_file()
                && let Ok(content) = fs::read_to_string(&path).await
            {
                for (i, line) in content.lines().enumerate() {
                    if re.is_match(line) {
                        matches.push(format!("{}:{}: {}", path.display(), i + 1, line));
                    }
                }
            }
        }

        Ok(GrepOutput { matches })
    }
}
