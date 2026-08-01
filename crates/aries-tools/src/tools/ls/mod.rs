mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSetBuilder};
use rig_agent::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::fs;

pub use self::args::LsArgs;
pub use self::error::LsError;
pub use self::output::LsOutput;

pub const NAME: &str = "Ls";

pub struct LsTool {
    cwd: PathBuf,
}

impl LsTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref().to_path_buf();

        Self { cwd }
    }
}

impl Tool for LsTool {
    const NAME: &'static str = NAME;
    type Args = LsArgs;
    type Output = LsOutput;
    type Error = LsError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
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
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
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
        Ok(LsOutput::new(entries))
    }
}
