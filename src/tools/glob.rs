use anyhow::Result;
use globset::GlobBuilder;
use ignore::WalkBuilder;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct GlobArgs {
    pattern: String,
}

#[derive(Serialize, Deserialize)]
pub struct GlobOutput {
    pub files: Vec<String>,
}

#[derive(thiserror::Error, Debug)]
pub enum GlobError {
    #[error("Glob pattern error: {0}")]
    PatternError(#[from] glob::PatternError),
    #[error("Globset error: {0}")]
    GlobsetError(#[from] globset::Error),
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

        // WalkBuilder automatically respects .gitignore files
        let mut builder = WalkBuilder::new(".");
        builder.hidden(false);

        let glob = GlobBuilder::new(&args.pattern).literal_separator(true).build()?;
        let glob = glob.compile_matcher();

        for result in builder.build() {
            if let Ok(entry) = result
                && entry.file_type().is_some_and(|ft| ft.is_file())
            {
                let path = entry.path();
                if glob.is_match(path) {
                    files.push(path.display().to_string());
                }
            }
        }

        Ok(GlobOutput { files })
    }
}
