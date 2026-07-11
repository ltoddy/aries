mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use globset::GlobBuilder;
use ignore::WalkBuilder;
use regex_lite::Regex;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use tokio::fs;

pub use self::args::GrepArgs;
pub use self::error::GrepError;
pub use self::output::GrepOutput;

pub const NAME: &str = "Grep";

pub struct GrepTool {
    cwd: PathBuf,
}

impl GrepTool {
    pub fn new(cwd: PathBuf) -> Self {
        Self { cwd }
    }
}

impl Tool for GrepTool {
    const NAME: &'static str = NAME;
    type Error = GrepError;
    type Args = GrepArgs;
    type Output = GrepOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("description.md").to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The regular expression to search for"
                    },
                    "include": {
                        "type": "string",
                        "description": "Optional glob pattern to filter files (e.g., src/**/*.rs)"
                    }
                },
                "required": ["pattern"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let re = Regex::new(&args.pattern)?;
        let mut matches = Vec::new();

        let mut builder = WalkBuilder::new(&self.cwd);
        builder.hidden(false);

        if let Some(include) = &args.include {
            let glob = GlobBuilder::new(include).literal_separator(true).build()?;
            let glob = glob.compile_matcher();
            let prefix = self.cwd.clone();
            builder.filter_entry(move |entry| {
                entry.file_type().is_some_and(|ft| ft.is_dir())
                    || entry
                        .path()
                        .strip_prefix(&prefix)
                        .map(|rel| glob.is_match(rel))
                        .unwrap_or(false)
            });
        }

        for result in builder.build() {
            if let Ok(entry) = result
                && entry.file_type().is_some_and(|ft| ft.is_file())
            {
                let path = entry.path();
                if let Ok(content) = fs::read_to_string(path).await {
                    for (i, line) in content.lines().enumerate() {
                        if re.is_match(line) {
                            matches.push(format!("{}:{}: {}", path.display(), i + 1, line));
                        }
                    }
                }
            }
        }

        Ok(GrepOutput { matches })
    }
}
