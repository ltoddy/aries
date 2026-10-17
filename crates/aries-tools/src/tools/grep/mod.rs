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
    ctx: crate::context::ToolContext,
}

impl GrepTool {
    pub fn new(ctx: crate::context::ToolContext) -> Self {
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
                    "description": "The regular expression to search for, matched one line at a time. Multiline patterns are not supported."
                },
                "include": {
                    "type": "string",
                    "description": "Optional glob pattern to filter files (e.g., src/**/*.rs)"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode: \"content\" shows matching lines (supports show_line_numbers and context), \"files_with_matches\" shows file paths sorted by mtime (default), \"count\" shows per-file matching line counts."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case insensitive search (rg -i). Defaults to false."
                },
                "show_line_numbers": {
                    "type": "boolean",
                    "description": "Show line numbers in content output (rg -n). Only applies to output_mode \"content\". Defaults to true."
                },
                "context": {
                    "type": "integer",
                    "minimum": 0,
                    "default": 0,
                    "description": "Number of lines to show on each side of a match (rg -C). Only applies to output_mode \"content\". Defaults to 0."
                },
                "hidden": {
                    "type": "boolean",
                    "description": "Include hidden files and directories. Defaults to false."
                },
                "respect_ignore": {
                    "type": "boolean",
                    "description": "Respect .gitignore and other ignore rules. Defaults to true."
                },
                "limit": {
                    "type": "integer",
                    "description": "Limit output to N matching lines across all files in \"content\" mode (context lines do not count), the N newest matching files in \"files_with_matches\" mode, or N files in \"count\" mode. Content and count results follow parallel traversal order. Defaults to 250. Pass 0 for unlimited."
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
