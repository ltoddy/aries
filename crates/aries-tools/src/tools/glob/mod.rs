mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use ignore::{WalkBuilder, WalkState};
use parking_lot::Mutex;
use rig::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::GlobArgs;
pub use self::error::GlobError;
pub use self::output::GlobOutput;

pub const NAME: &str = "Glob";

const MAX_RESULTS: usize = 100;

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
                "respect_gitignore": {
                    "type": "boolean",
                    "description": "Respect .gitignore and other ignore rules. Defaults to true."
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

        let mut walker = WalkBuilder::new(&base_dir);
        walker
            .hidden(!args.hidden)
            .git_ignore(args.respect_gitignore)
            .git_exclude(args.respect_gitignore)
            .git_global(args.respect_gitignore)
            .ignore(args.respect_gitignore);

        let matches = Arc::new(Mutex::new(Vec::<(PathBuf, SystemTime)>::new()));
        let base_dir = Arc::new(base_dir);
        let set = Arc::new(set);

        walker.build_parallel().run(|| {
            let base_dir = base_dir.clone();
            let matches = matches.clone();
            let set = set.clone();

            Box::new(move |entry| {
                let Ok(entry) = entry else { return WalkState::Continue };

                let Ok(relative) = entry.path().strip_prefix(base_dir.as_path()) else {
                    return WalkState::Continue;
                };

                if !set.is_match(relative) {
                    return WalkState::Continue;
                }

                let modified = entry
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .unwrap_or(SystemTime::UNIX_EPOCH);

                matches.lock().push((relative.to_owned(), modified));
                WalkState::Continue
            })
        });

        let mut matches = Arc::into_inner(matches).expect("glob matches still shared").into_inner();

        matches.sort_by_key(|next| std::cmp::Reverse(next.1));

        let truncated = matches.len() > MAX_RESULTS;
        let files =
            matches.into_iter().take(MAX_RESULTS).map(|(filename, _)| filename).collect::<Vec<_>>();

        Ok(GlobOutput::new(files, truncated))
    }
}
