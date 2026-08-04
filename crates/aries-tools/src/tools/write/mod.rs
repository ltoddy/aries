mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use rig_agent::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::fs;

pub use self::args::WriteArgs;
pub use self::error::WriteError;
pub use self::output::{WriteKind, WriteOutput};
use crate::tools::diff;

pub const NAME: &str = "Write";

pub struct WriteTool {
    cwd: PathBuf,
    ctx: crate::context::ToolContext,
}

impl WriteTool {
    pub fn new(cwd: impl AsRef<Path>, ctx: crate::context::ToolContext) -> Self {
        let cwd = cwd.as_ref().to_path_buf();

        Self { cwd, ctx }
    }
}

impl Tool for WriteTool {
    const NAME: &'static str = NAME;
    type Args = WriteArgs;
    type Output = WriteOutput;
    type Error = WriteError;

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

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let file_path = if args.file_path.is_relative() {
            self.cwd.join(&args.file_path)
        } else {
            args.file_path
        };

        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        if let Ok(metadata) = fs::metadata(&file_path).await
            && metadata.len() > 0
        {
            return Err(WriteError::file_not_empty(&file_path));
        }

        let original_content = fs::read_to_string(&file_path).await.ok();
        if let Some(ref original_content) = original_content {
            let _ = self.ctx.file_checkpoint.push(&file_path, original_content).await;
        }

        fs::write(&file_path, &args.content).await?;
        self.ctx.on_file_written(&file_path, &args.content).await;

        match original_content {
            Some(original_content) => {
                let (hunks, additions, deletions) = diff::diff(&original_content, &args.content);

                let output = WriteOutput::for_update(
                    file_path,
                    original_content,
                    hunks,
                    additions,
                    deletions,
                );
                Ok(output)
            },
            None => {
                let additions = args.content.lines().count();
                let output = WriteOutput::for_create(file_path, additions);
                Ok(output)
            },
        }
    }
}
