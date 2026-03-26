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

        println!("\n{} Aries wants to run the following command:", "⚠️  WARNING".yellow().bold());
        println!("  {}", command.cyan());
        print!("Do you want to allow this? [y/N] ");
        std::io::Write::flush(&mut std::io::stdout()).unwrap();

        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();

        if input.trim().to_lowercase() != "y" {
            return Err(ShellCommandError::ExecutionFailed("User denied execution".to_string()));
        }

        let output = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| ShellCommandError::ExecutionFailed(e.to_string()))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

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
