mod args;
mod driver;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use driver::{Collector, Query, StopState, walk_parallel};
use grep_regex::RegexMatcherBuilder;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::{GrepArgs, OutputMode};
pub use self::error::GrepError;
pub use self::output::GrepOutput;

pub const NAME: &str = "Grep";

pub struct GrepTool {
    cwd: PathBuf,
}

impl GrepTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref().to_owned();
        Self { cwd }
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
                    "description": "The regular expression to search for"
                },
                "include": {
                    "type": "string",
                    "description": "Optional glob pattern to filter files (e.g., src/**/*.rs)"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["content", "files_with_matches", "count"],
                    "description": "Output mode: \"content\" shows matching lines (supports show_line_numbers and context_before/context_after/context), \"files_with_matches\" shows file paths sorted by mtime (default), \"count\" shows per-file match counts."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case insensitive search (rg -i). Defaults to false."
                },
                "show_line_numbers": {
                    "type": "boolean",
                    "description": "Show line numbers in content output (rg -n). Only applies to output_mode \"content\". Defaults to true."
                },
                "context_before": {
                    "type": "integer",
                    "description": "Number of lines to show before each match (rg -B). Only applies to output_mode \"content\"."
                },
                "context_after": {
                    "type": "integer",
                    "description": "Number of lines to show after each match (rg -A). Only applies to output_mode \"content\"."
                },
                "context": {
                    "type": "integer",
                    "description": "Number of lines to show before and after each match (rg -C). Takes precedence over context_before/context_after. Only applies to output_mode \"content\"."
                },
                "respect_gitignore": {
                    "type": "boolean",
                    "description": "Respect .gitignore and other ignore rules. Defaults to true."
                },
                "head_limit": {
                    "type": "integer",
                    "description": "Limit output to the first N match groups in \"content\" mode, or the first N entries in \"files_with_matches\" and \"count\" modes. Defaults to 250. Pass 0 for unlimited."
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
        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(args.case_insensitive)
            .build(&args.pattern)?;
        let stop = Arc::new(StopState::new(args.head_limit));
        let collector = Arc::new(Collector::new(args.output_mode, stop));

        let query = Query::from(args);

        walk_parallel(&self.cwd, query, matcher, collector.clone())?;
        let collector = Arc::into_inner(collector).ok_or(GrepError::CollectorStillShared)?;
        let (matches, truncated) = collector.finish();

        Ok(GrepOutput::new(matches, truncated))
    }
}
