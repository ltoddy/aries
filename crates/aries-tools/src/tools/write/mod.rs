mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::fs;

pub use self::args::WriteArgs;
pub use self::error::{WriteError, WriteOperation};
pub use self::output::WriteOutput;
pub const NAME: &str = "Write";

pub struct WriteTool {
    ctx: crate::context::ToolContext,
}

impl WriteTool {
    pub fn new(ctx: crate::context::ToolContext) -> Self {
        Self { ctx }
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
        let cwd = self
            .ctx
            .worktree()
            .map_or_else(|| self.ctx.cwd(), |worktree| worktree.path().to_owned());
        let file_path =
            if args.file_path.is_relative() { cwd.join(&args.file_path) } else { args.file_path };

        match fs::metadata(&file_path).await {
            Ok(metadata) if metadata.len() > 0 => {
                return Err(WriteError::file_not_empty(file_path.clone()));
            },
            Ok(_) => {
                self.ctx.guard_write(&file_path).await?;
                let _ = self.ctx.file_checkpoint.push(&file_path, "").await;
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if let Some(parent) = file_path.parent() {
                    fs::create_dir_all(parent).await.map_err(|err| {
                        WriteError::io(
                            WriteOperation::CreateParentDirectory,
                            file_path.clone(),
                            err,
                        )
                    })?;
                }
            },
            Err(err) => {
                return Err(WriteError::io(WriteOperation::ReadMetadata, file_path.clone(), err));
            },
        }

        fs::write(&file_path, &args.content)
            .await
            .map_err(|err| WriteError::io(WriteOperation::WriteFile, file_path.clone(), err))?;
        self.ctx.on_file_written(&file_path, &args.content).await;

        let additions = args.content.lines().count();
        let output = WriteOutput::new(file_path, additions);
        Ok(output)
    }
}
