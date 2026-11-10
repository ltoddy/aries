mod args;
mod driver;
mod error;
mod output;
#[cfg(test)]
mod tests;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::{GrepArgs, OutputMode};
pub use self::error::GrepError;
pub use self::output::GrepOutput;

pub const NAME: &str = "Grep";

pub struct GrepTool {
    ctx: crate::tools::context::ToolContext,
}

impl GrepTool {
    pub fn new(ctx: crate::tools::context::ToolContext) -> Self {
        Self { ctx }
    }
}

impl Tool for GrepTool {
    const NAME: &'static str = NAME;
    type Args = GrepArgs;
    type Output = GrepOutput;
    type Error = GrepError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression using Rust regex syntax. Escape metacharacters for literal text; multiline matches, lookaround and backreferences are unsupported."
                },
                "include": {
                    "type": "string",
                    "description": "Optional path glob relative to the current working directory, e.g. src/**/*.rs. Filters files without changing the search root."
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "files_with_matches (default) returns matching file paths; content returns matching lines with optional context; count returns matching-line counts per file, not occurrence counts."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Match without case sensitivity. Defaults to false."
                },
                "show_line_numbers": {
                    "type": "boolean",
                    "description": "Include 1-based line numbers in content output. Defaults to true; ignored in other modes."
                },
                "context": {
                    "type": "integer",
                    "minimum": 0,
                    "default": 0,
                    "description": "Lines before and after each match in content mode; ignored in other modes and excluded from the match limit."
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
                    "description": "Maximum matching lines across all files in content mode (excluding context), or files in other modes. Defaults to 250; 0 is unlimited."
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
        let cwd = self
            .ctx
            .worktree()
            .map_or_else(|| self.ctx.cwd(), |worktree| worktree.path().to_owned());
        tokio::task::spawn_blocking(move || driver::run(&cwd, &args))
            .await
            .map_err(GrepError::task)?
    }
}
