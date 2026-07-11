mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::process::Stdio;

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use tokio::process::Command;

pub use self::args::BashArgs;
pub use self::error::BashError;
pub use self::output::BashOutput;

pub const NAME: &str = "Bash";

pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        Self {}
    }

    const SHELL_OPERATORS: &[&str] = &["&&", "||", "|", ";", "&"];

    fn rewrite_last_command(cmd: impl Into<String>) -> String {
        let cmd = cmd.into();
        let tokens = cmd.split_whitespace().collect::<Vec<_>>();
        let insert_at =
            tokens.iter().rposition(|t| Self::SHELL_OPERATORS.contains(t)).map_or(0, |i| i + 1);

        let mut result = Vec::with_capacity(tokens.len() + 2);
        result.extend_from_slice(&tokens[..insert_at]);
        result.push("aries exec");
        result.extend_from_slice(&tokens[insert_at..]);
        result.join(" ")
    }
}

impl Tool for BashTool {
    const NAME: &'static str = NAME;
    type Error = BashError;
    type Args = BashArgs;
    type Output = BashOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("description.md").to_owned(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let command = Self::rewrite_last_command(args.command);

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_owned());
        let output = Command::new(shell)
            .arg("-c")
            .arg(&command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| BashError::ExecutionFailed(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(BashOutput { stdout, stderr, exit_code })
    }
}
