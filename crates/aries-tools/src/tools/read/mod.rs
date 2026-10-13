mod args;
mod error;
mod inspector;
mod output;
#[cfg(test)]
mod tests;

use std::io::SeekFrom;

use rig::tool::{Tool, ToolContext};
use serde_json::Value;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader};

pub use self::args::ReadArgs;
pub use self::error::{ReadError, ReadOperation};
pub use self::output::ReadOutput;

pub const NAME: &str = "Read";

const MAX_LINES_TO_READ: usize = 2000;
const EMPTY_FILE_NOTICE: &str =
    "<system-reminder>File exists but has empty contents.</system-reminder>";
const SEPARATOR: char = '→';

pub struct ReadTool {
    ctx: crate::context::ToolContext,
}

impl ReadTool {
    pub fn new(ctx: crate::context::ToolContext) -> Self {
        Self { ctx }
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
        let cwd = self
            .ctx
            .worktree()
            .map_or_else(|| self.ctx.cwd(), |worktree| worktree.path().to_owned());
        let file_path =
            if args.file_path.is_relative() { cwd.join(&args.file_path) } else { args.file_path };

        if file_path.is_dir() {
            return Err(ReadError::is_a_directory(file_path));
        }

        let mut file = fs::File::open(&file_path)
            .await
            .map_err(|err| ReadError::io(ReadOperation::OpenFile, file_path.clone(), err))?;
        let metadata = file
            .metadata()
            .await
            .map_err(|err| ReadError::io(ReadOperation::ReadMetadata, file_path.clone(), err))?;
        if metadata.len() == 0 {
            self.ctx.on_file_read(&file_path).await;
            return Ok(ReadOutput::new(EMPTY_FILE_NOTICE, false));
        }

        let mut probe = vec![0u8; inspector::MAX_SCAN_SIZE];
        let sniffed = file
            .read(&mut probe)
            .await
            .map_err(|err| ReadError::io(ReadOperation::InspectFileType, file_path.clone(), err))?;
        if inspector::inspect(&probe[..sniffed]).is_binary() {
            return Err(ReadError::binary_file(file_path));
        }
        file.seek(SeekFrom::Start(0))
            .await
            .map_err(|err| ReadError::io(ReadOperation::SeekToStart, file_path.clone(), err))?;

        let reader = BufReader::new(file);

        let start_at = args.offset.map(|offset| offset.max(1)).unwrap_or(1);
        let skip = start_at - 1;
        let limit = args.limit.unwrap_or(MAX_LINES_TO_READ);
        if limit == 0 {
            return Err(ReadError::invalid_limit());
        }

        let mut lines = reader.lines();
        let mut content_lines = Vec::<String>::new();
        let mut line_no: usize = 0;
        while let Some(line) = lines
            .next_line()
            .await
            .map_err(|err| ReadError::io(ReadOperation::ReadLine, file_path.clone(), err))?
        {
            line_no += 1;
            if line_no <= skip {
                continue;
            }
            content_lines.push(format!("{:>6}{SEPARATOR}{line}", line_no));
            if content_lines.len() >= limit {
                break;
            }
        }

        let truncated = lines
            .next_line()
            .await
            .map_err(|err| ReadError::io(ReadOperation::CheckTruncated, file_path.clone(), err))?
            .is_some();
        let content = content_lines.join("\n");
        self.ctx.on_file_read(&file_path).await;

        Ok(ReadOutput::new(content, truncated))
    }
}
