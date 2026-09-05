mod args;
mod driver;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use driver::{Collector, walk_parallel};
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::GlobArgs;
pub use self::error::GlobError;
pub use self::output::GlobOutput;

pub const NAME: &str = "Glob";

pub struct GlobTool {
    cwd: PathBuf,
}

impl GlobTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref().to_owned();

        Self { cwd }
    }
}

impl Tool for GlobTool {
    const NAME: &'static str = NAME;
    type Args = GlobArgs;
    type Output = GlobOutput;
    type Error = GlobError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match against (e.g., src/**/*.rs)"
                },
                "base_dir": {
                    "type": "string",
                    "description": "Base directory for the glob pattern"
                },
                "hidden": {
                    "type": "boolean",
                    "description": "Include hidden files (starting with '.'). Defaults to false."
                },
                "respect_ignore": {
                    "type": "boolean",
                    "description": "Respect .gitignore and other ignore rules. Defaults to true."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of matched paths to return. Defaults to 100. Pass 0 for unlimited."
                }
            },
            "required": ["pattern"]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
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
        let collector = Arc::new(Collector::new(args.limit));

        walk_parallel(&base_dir, args.hidden, args.respect_ignore, set, collector.clone())?;

        let collector = Arc::into_inner(collector).ok_or(GlobError::CollectorStillShared)?;
        let (files, truncated) = collector.finish();

        Ok(GlobOutput::new(files, truncated))
    }
}
