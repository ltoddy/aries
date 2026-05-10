use std::process::Stdio;

use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::tools::{RenderError, ToolArgsRender, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct BashArgs {
    pub command: String,
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

impl Tool for BashTool {
    const NAME: &'static str = NAME;
    type Error = BashError;
    type Args = BashArgs;
    type Output = BashOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/bash.txt").to_string(),
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
        let command = args.command;

        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| BashError::ExecutionFailed(e.to_string()))?;

        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Helper function to truncate string to max lines
        fn truncate_lines(s: &mut String, max_lines: usize) {
            let line_count = s.lines().count();
            if line_count > max_lines {
                let truncated: Vec<&str> = s.lines().take(max_lines).collect();
                *s = format!(
                    "{}\n\n... ({} more lines truncated)",
                    truncated.join("\n"),
                    line_count - max_lines
                );
            }
        }

        // Limit the number of lines returned to the LLM and printed to terminal
        truncate_lines(&mut stdout, 200);
        truncate_lines(&mut stderr, 200);

        Ok(BashOutput { stdout, stderr, exit_code: output.status.code().unwrap_or(-1) })
    }
}
