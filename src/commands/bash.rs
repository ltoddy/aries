use std::process::Stdio;

use colored::Colorize;
use tokio::process::Command;

pub const NAME: &str = "/bash";

pub async fn execute(command: &str) {
    let command = command.trim();
    if command.is_empty() {
        eprintln!("{}", format!("No command provided after {NAME}").red());
        return;
    }

    match Command::new("bash")
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
