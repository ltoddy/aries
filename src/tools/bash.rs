use std::process::Stdio;

use colored::Colorize;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

#[derive(Deserialize)]
pub struct ShellCommandArgs {
    command: String,
}

#[derive(Serialize)]
pub struct ShellCommandOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

#[derive(thiserror::Error, Debug)]
pub enum ShellCommandError {
    #[error("Failed to execute command: {0}")]
    ExecutionFailed(String),
}

pub struct ShellCommand;

impl Tool for ShellCommand {
    const NAME: &'static str = "shell_command";
    type Error = ShellCommandError;
    type Args = ShellCommandArgs;
    type Output = ShellCommandOutput;

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
            .map_err(|e| ShellCommandError::ExecutionFailed(e.to_string()))?;

        let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Helper function to truncate string to max lines
        fn truncate_lines(s: &mut String, max_lines: usize) {
            let line_count = s.lines().count();
            if line_count > max_lines {
                let truncated: Vec<&str> = s.lines().take(max_lines).collect();
                *s = format!("{}\n\n... ({} more lines truncated)", truncated.join("\n"), line_count - max_lines);
            }
        }

        // Limit the number of lines returned to the LLM and printed to terminal
        truncate_lines(&mut stdout, 10);
        truncate_lines(&mut stderr, 10);

        // Also print output to terminal so user can see what happened
        if !stdout.is_empty() {
            println!("{}", stdout.dimmed());
        }
        if !stderr.is_empty() {
            eprintln!("{}", stderr.red());
        }

        Ok(ShellCommandOutput { stdout, stderr, exit_code: output.status.code().unwrap_or(-1) })
    }
}
