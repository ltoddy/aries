mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use rig_core::tool::Tool;
use serde_json::Value;
use similar::TextDiff;
use tokio::fs;

pub use self::args::WriteArgs;
pub use self::error::WriteError;
pub use self::output::{Hunk, WriteKind, WriteOutput};
use crate::context::ToolContext;

pub const NAME: &str = "Write";

pub struct WriteTool {
    _cwd: PathBuf,
    ctx: ToolContext,
}

impl WriteTool {
    pub fn new(cwd: impl AsRef<Path>, ctx: ToolContext) -> Self {
        let cwd = cwd.as_ref().to_path_buf();

        Self { _cwd: cwd, ctx }
    }
}

impl Tool for WriteTool {
    const NAME: &'static str = NAME;
    type Error = WriteError;
    type Args = WriteArgs;
    type Output = WriteOutput;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to write (absolute or relative to the working directory)"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        if let Some(parent) = args.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let original_content = fs::read_to_string(&args.file_path).await.ok();

        fs::write(&args.file_path, &args.content).await?;
        self.ctx.on_file_written(&args.file_path, &args.content).await;

        match original_content {
            Some(original_content) => {
                let diff = TextDiff::from_lines(&original_content, &args.content);

                let mut additions = 0_usize;
                let mut deletions = 0_usize;
                let mut hunks = Vec::new();
                for op in diff.ops() {
                    match op.tag() {
                        similar::DiffTag::Equal => continue,
                        similar::DiffTag::Insert => additions += op.new_range().len(),
                        similar::DiffTag::Delete => deletions += op.old_range().len(),
                        similar::DiffTag::Replace => {
                            deletions += op.old_range().len();
                            additions += op.new_range().len();
                        },
                    }
                    hunks.push(Hunk::from_diff(op, &diff));
                }

                let output = WriteOutput::for_update(
                    args.file_path,
                    original_content,
                    hunks,
                    additions,
                    deletions,
                );
                Ok(output)
            },
            None => {
                let additions = args.content.lines().count();
                let output = WriteOutput::for_create(args.file_path, additions);
                Ok(output)
            },
        }
    }
}
