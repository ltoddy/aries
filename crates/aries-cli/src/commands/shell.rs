use std::path::PathBuf;
use std::process::Stdio;

use aries_tools::shell::detect_shell;
use colored::Colorize;

pub async fn execute(command: &str) {
    let command = command.trim();
    if command.is_empty() {
        eprintln!("{}", "No command provided after /shell".red());
        return;
    }

    let shell = detect_shell();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut cmd = shell.build_command(command, &cwd);
    match cmd
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
