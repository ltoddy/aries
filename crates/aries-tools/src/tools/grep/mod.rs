mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use globset::GlobBuilder;
use ignore::WalkBuilder;
use regex_lite::RegexBuilder;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::fs;

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
                "head_limit": {
                    "type": "integer",
                    "description": "Limit output to the first N lines/entries across all modes. Defaults to 250. Pass 0 for unlimited."
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
        let re =
            RegexBuilder::new(&args.pattern).case_insensitive(args.case_insensitive).build()?;

        let (before, after) = match args.context {
            Some(c) => (c, c),
            None => (args.context_before.unwrap_or(0), args.context_after.unwrap_or(0)),
        };

        let mut builder = WalkBuilder::new(&self.cwd);
        builder.hidden(false);

        if let Some(include) = &args.include {
            let glob = GlobBuilder::new(include).literal_separator(true).build()?;
            let matcher = glob.compile_matcher();
            let prefix = self.cwd.clone();
            builder.filter_entry(move |entry| {
                entry.file_type().is_some_and(|ft| ft.is_dir())
                    || entry
                        .path()
                        .strip_prefix(&prefix)
                        .map(|rel| matcher.is_match(rel))
                        .unwrap_or(false)
            });
        }

        let mut content_lines = Vec::<String>::new();
        let mut file_entries = Vec::<(PathBuf, SystemTime)>::new();
        let mut count_lines = Vec::<String>::new();

        for entry in builder.build() {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_some_and(|f| f.is_file()) {
                continue;
            }

            let path = entry.path();
            let Ok(content) = fs::read_to_string(path).await else { continue };

            let lines = content.lines().collect::<Vec<_>>();
            let match_indices = lines
                .iter()
                .enumerate()
                .filter_map(|(i, line)| if re.is_match(line) { Some(i) } else { None })
                .collect::<Vec<_>>();
            if match_indices.is_empty() {
                continue;
            }

            let relative_path = path.strip_prefix(&self.cwd).unwrap_or(path);
            let path_display = relative_path.display().to_string();
            match args.output_mode {
                OutputMode::Content => {
                    let mut emit = BTreeSet::<usize>::new();

                    for index in &match_indices {
                        let start = index.saturating_sub(before);
                        let end = index.saturating_add(after).saturating_add(1).min(lines.len());
                        emit.extend(start..end);
                    }

                    for i in emit {
                        let is_match = match_indices.binary_search(&i).is_ok();
                        let sep = if is_match { ':' } else { '-' };
                        let rendered = if args.show_line_numbers {
                            format!("{path_display}{sep}{}{sep}{}", i + 1, lines[i])
                        } else {
                            format!("{path_display}{sep}{}", lines[i])
                        };
                        content_lines.push(rendered);
                    }
                },
                OutputMode::FilesWithMatches => {
                    let modified = entry
                        .metadata()
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    file_entries.push((relative_path.to_owned(), modified));
                },
                OutputMode::Count => {
                    count_lines.push(format!("{path_display}:{}", match_indices.len()));
                },
            }
        }

        let mut matches = match args.output_mode {
            OutputMode::Content => content_lines,
            OutputMode::FilesWithMatches => {
                file_entries.sort_by_key(|(_, mtime)| std::cmp::Reverse(*mtime));
                file_entries.into_iter().map(|(path, _)| path.display().to_string()).collect()
            },
            OutputMode::Count => count_lines,
        };

        let truncated = args.head_limit != 0 && matches.len() > args.head_limit;
        if args.head_limit != 0 {
            matches.truncate(args.head_limit);
        }

        Ok(GrepOutput::new(matches, truncated))
    }
}
