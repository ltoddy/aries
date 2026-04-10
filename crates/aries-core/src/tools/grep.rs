use anyhow::Result;
use globset::GlobBuilder;
use ignore::WalkBuilder;
use regex::Regex;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::fs;

#[derive(Debug, Deserialize, Serialize)]
pub struct GrepArgs {
    pub pattern: String,
    pub include: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GrepOutput {
    pub matches: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum GrepError {
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
    #[error("Glob error: {0}")]
    Glob(#[from] glob::PatternError),
    #[error("Globset error: {0}")]
    Globset(#[from] globset::Error),
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

        // WalkBuilder automatically respects .gitignore files
        let mut builder = WalkBuilder::new(".");
        builder.hidden(false); // Don't skip hidden files by default

        // Add glob filter if specified
        if let Some(include) = &args.include {
            let glob = GlobBuilder::new(include).literal_separator(true).build()?;
            let glob = glob.compile_matcher();
            builder.filter_entry(move |entry| glob.is_match(entry.path()));
        }

        for result in builder.build() {
            if let Ok(entry) = result
                && entry.file_type().is_some_and(|ft| ft.is_file())
            {
                let path = entry.path();
                if let Ok(content) = fs::read_to_string(path).await {
                    for (i, line) in content.lines().enumerate() {
                        if re.is_match(line) {
                            matches.push(format!("{}:{}: {}", path.display(), i + 1, line));
                        }
                    }
                }
            }
        }

        Ok(GrepOutput { matches })
    }
}
