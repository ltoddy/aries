mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use rig_core::tool::Tool;
use serde_json::Value;
use tokio::fs;

pub use self::args::ReadArgs;
pub use self::error::ReadError;
pub use self::output::ReadOutput;

pub const NAME: &str = "Read";

pub struct ReadTool;

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ReadTool {
    pub fn new() -> Self {
        Self {}
    }
}

impl Tool for ReadTool {
    const NAME: &'static str = NAME;
    type Error = ReadError;
    type Args = ReadArgs;
    type Output = ReadOutput;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to read"
                },
                "offset": {
                    "type": "number",
                    "description": "The line number to start reading from (1-indexed)"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut content = fs::read_to_string(&args.file_path).await?;

        let offset = args.offset.map(|offset| offset.saturating_sub(1));
        if let Some(offset) = offset {
            content = content.lines().skip(offset).collect::<Vec<_>>().join("\n");
        }

        Ok(ReadOutput { content })
    }
}
