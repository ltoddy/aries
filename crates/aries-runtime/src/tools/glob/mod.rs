mod args;
mod driver;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::Path;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::GlobArgs;
use self::driver::{Collector, walk_parallel};
pub use self::error::GlobError;
pub use self::output::GlobOutput;

pub const NAME: &str = "Glob";

pub struct GlobTool {
    ctx: crate::tools::context::ToolContext,
}

impl GlobTool {
    pub fn new(ctx: crate::tools::context::ToolContext) -> Self {
        Self { ctx }
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
                    "description": "Path glob relative to base_dir, e.g. **/*.rs or src/**/*.ts. Absolute patterns under base_dir are converted to relative patterns; prefer an explicit base_dir with a relative pattern."
                },
                "base_dir": {
                    "type": "string",
                    "description": "Directory to search recursively. Defaults to the current working directory (the active worktree, if any); relative directories resolve from there."
                },
                "hidden": {
                    "type": "boolean",
                    "description": "Include hidden files and directories. Defaults to false; ignore rules still apply."
                },
                "respect_ignore": {
                    "type": "boolean",
                    "description": "Honor .gitignore and other ignore rules. Defaults to true; independent of hidden-file filtering."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of matching file paths to return. Defaults to 100; 0 is unlimited."
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
        let current_dir = self
            .ctx
            .worktree()
            .map_or_else(|| self.ctx.cwd(), |worktree| worktree.path().to_owned());
        let base_dir = match args.base_dir {
            Some(base_dir) if base_dir.is_absolute() => base_dir,
            Some(base_dir) => current_dir.join(base_dir),
            None => current_dir,
        };

        let pattern = if Path::new(&args.pattern).is_absolute() {
            match Path::new(&args.pattern).strip_prefix(&base_dir) {
                Ok(rel) => rel.to_string_lossy().to_string(),
                Err(_) => args.pattern,
            }
        } else {
            args.pattern
        };

        let glob =
            globset::Glob::new(&pattern).map_err(|err| GlobError::pattern(pattern.clone(), err))?;
        let set = globset::GlobSetBuilder::new()
            .add(glob)
            .build()
            .map_err(|err| GlobError::glob_set(pattern.clone(), err))?;
        let collector = Collector::new(args.limit);

        walk_parallel(&base_dir, args.hidden, args.respect_ignore, &set, &collector)?;
        let (files, truncated) = collector.finish();

        Ok(GlobOutput::new(files, truncated))
    }
}
