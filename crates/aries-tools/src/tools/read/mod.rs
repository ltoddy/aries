mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use rig::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};

pub use self::args::ReadArgs;
pub use self::error::ReadError;
pub use self::output::ReadOutput;

pub const NAME: &str = "Read";

const MAX_LINES_TO_READ: usize = 2000;
const EMPTY_FILE_NOTICE: &str =
    "<system-reminder>File exists but has empty contents.</system-reminder>";
const SEPARATOR: char = '→';

pub struct ReadTool {
    cwd: PathBuf,
    ctx: crate::context::ToolContext,
}

impl ReadTool {
    pub fn new(cwd: impl AsRef<Path>, ctx: crate::context::ToolContext) -> Self {
        let cwd = cwd.as_ref().to_path_buf();

        Self { cwd, ctx }
    }
}

impl Tool for ReadTool {
    const NAME: &'static str = NAME;
    type Args = ReadArgs;
    type Output = ReadOutput;
    type Error = ReadError;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to read (absolute or relative to the working directory)"
                },
                "offset": {
                    "type": "number",
                    "description": "The line number to start reading from (1-indexed). Only provide if the file is too large to read at once"
                },
                "limit": {
                    "type": "number",
                    "description": "The number of lines to read. Only provide if the file is too large to read at once"
                }
            },
            "required": ["file_path"]
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

        if file_path.is_dir() {
            return Err(ReadError::is_a_directory(file_path));
        }

        let file = fs::File::open(&file_path).await?;
        let metadata = file.metadata().await?;
        if metadata.len() == 0 {
            self.ctx.on_file_read(&file_path).await;
            return Ok(ReadOutput { content: EMPTY_FILE_NOTICE.to_owned() });
        }

        let reader = BufReader::new(file);

        let start_at = args.offset.map(|offset| offset.max(1)).unwrap_or(1);
        let skip = start_at - 1;
        let limit = args.limit.unwrap_or(MAX_LINES_TO_READ);

        let mut lines = reader.lines();
        let mut content_lines = Vec::<String>::new();
        let mut line_no: usize = 0;
        while let Some(line) = lines.next_line().await? {
            line_no += 1;
            if line_no <= skip {
                continue;
            }
            content_lines.push(format!("{:>6}{SEPARATOR}{line}", line_no));
            if content_lines.len() >= limit {
                break;
            }
        }

        let content = content_lines.join("\n");
        self.ctx.on_file_read(&file_path).await;

        Ok(ReadOutput { content })
    }
}
