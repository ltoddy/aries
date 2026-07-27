use std::process::Stdio;

use colored::Colorize;
use tokio::process::Command;

pub async fn execute(command: &str) {
    let command = command.trim();
    if command.is_empty() {
        eprintln!("{}", "No command provided after /shell".red());
        return;
    }

    let shell = std::env::var("SHELL").unwrap_or(String::from("bash"));
    match Command::new(shell)
        .arg("-c")
        .arg(command)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await
    {
        Ok(status) => {
            if !status.success() {
                eprintln!("{}", format!("Command failed with status: {}", status).red());
            }
        },
        Err(e) => {
            eprintln!("{}", format!("Failed to execute command: {}", e).red());
        },
    }
}
