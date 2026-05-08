use std::fmt::{self, Display};
use std::path::{Path, PathBuf};

use anyhow::Result;
use ignore::WalkBuilder;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobArgs {
    pub pattern: String,
    pub base_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GlobOutput {
    pub files: Vec<String>,
}

impl Display for GlobOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.files.join("\n"))
    }
}

#[derive(thiserror::Error, Debug)]
pub enum GlobError {
    #[error("Glob pattern error: {0}")]
    PatternError(#[from] glob::PatternError),
    #[error("Globset error: {0}")]
    GlobsetError(#[from] globset::Error),
    #[error("Walk error: {0}")]
    Walk(String),
}

pub const NAME: &str = "Glob";

pub struct GlobTool {
    cwd: PathBuf,
}

impl GlobTool {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }
}

impl Tool for GlobTool {
    const NAME: &'static str = NAME;
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
                    },
                    "base_dir": {
                        "type": "string",
                        "description": "Base directory for the glob pattern"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let base_dir = args.base_dir.unwrap_or_else(|| self.cwd.clone());

        let pattern = if Path::new(&args.pattern).is_absolute() {
            match Path::new(&args.pattern).strip_prefix(&base_dir) {
                Ok(rel) => rel.to_string_lossy().to_string(),
                Err(_) => args.pattern,
            }
        } else {
            args.pattern
        };

        let pattern = globset::Glob::new(&pattern)?;
        let set = globset::GlobSetBuilder::new().add(pattern).build()?;

        let mut walker = WalkBuilder::new(&base_dir);
        walker.hidden(true).ignore(true);

        let files = tokio::task::spawn_blocking(move || -> Vec<String> {
            let mut matches = Vec::<_>::new();
            for entry in walker.build() {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(_) => continue,
                };

                let relative = match entry.path().strip_prefix(&base_dir) {
                    Ok(relative) => relative,
                    Err(_) => continue,
                };

                if !set.is_match(relative) {
                    continue;
                }

                let filename = match relative.to_str().map(ToOwned::to_owned) {
                    Some(r) => r,
                    None => continue,
                };

                matches.push(filename);
            }
            matches
        })
        .await
        .map_err(|err| GlobError::Walk(err.to_string()))?;

        Ok(GlobOutput { files })
    }
}
