mod args;
mod error;
mod output;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use rig_core::tool::Tool;
use serde_json::Value;
use tokio::process::Command;

pub use self::args::BashArgs;
pub use self::error::BashError;
pub use self::output::BashOutput;

pub const NAME: &str = "Bash";

pub struct BashTool {
    cwd: PathBuf,
}

impl BashTool {
    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref().to_path_buf();
        Self { cwd }
    }

    const DEFAULT_TIMEOUT_MS: u64 = 120_000;
    const MAX_TIMEOUT_MS: u64 = 600_000;
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

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": format!(
                        "Optional timeout in milliseconds (default {}, max {})",
                        Self::DEFAULT_TIMEOUT_MS,
                        Self::MAX_TIMEOUT_MS
                    )
                },
                "description": {
                    "type": "string",
                    "description": "Clear, concise description of what this command does in 5-10 words"
                }
            },
            "required": ["command"]
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let arg = Self::rewrite_last_command(args.command);

        let timeout = args
            .timeout
            .filter(|&ms| ms > 0)
            .unwrap_or(Self::DEFAULT_TIMEOUT_MS)
            .min(Self::MAX_TIMEOUT_MS);
        let timeout = Duration::from_millis(timeout);

        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_owned());
        let mut command = Command::new(shell);
        command
            .arg("-c")
            .arg(arg)
            .current_dir(&self.cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let output = match tokio::time::timeout(timeout, command.output()).await {
            Ok(result) => result.map_err(BashError::Io)?,
            Err(_) => return Err(BashError::Timeout(timeout.as_millis() as u64)),
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        Ok(BashOutput::new(stdout, stderr, exit_code))
    }
}
