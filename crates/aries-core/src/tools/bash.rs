use std::process::Stdio;

use anyhow::Result;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::tools::{RenderError, ToolArgsRender, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct BashArgs {
    pub command: String,
}

impl BashArgs {
    pub fn title(&self) -> String {
        let command = self.command.trim();
        if command.is_empty() {
            "Run a shell command".to_owned()
        } else {
            format!("Run shell command: {command}")
        }
    }
}

impl ToolArgsRender for BashArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.command;
        Ok((first, None))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BashOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl ToolOutputRender for BashOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        let mut text = String::new();
        if !output.stdout.is_empty() {
            text.push_str(&output.stdout);
        }
        if !output.stderr.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("stderr: {}", output.stderr));
        }
        if output.exit_code != 0 {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(&format!("exit_code: {}", output.exit_code));
        }
        Ok(text)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum BashError {
    #[error("Failed to execute command: {0}")]
    ExecutionFailed(String),
}

pub const NAME: &str = "Bash";

pub struct BashTool;

impl BashTool {
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
            description: include_str!("bash.md").to_owned(),
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
