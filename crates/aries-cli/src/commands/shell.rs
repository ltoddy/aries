use std::process::Stdio;

use tokio::process::Command;

use crate::theme::Theme;

pub async fn execute(command: &str, theme: &Theme) {
    let command = command.trim();
    if command.is_empty() {
        eprintln!("{}", theme.red_text("No command provided after /shell"));
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
                eprintln!("{}", theme.red_text(&format!("Command failed with status: {}", status)));
            }
        },
        Err(e) => {
            eprintln!("{}", theme.red_text(&format!("Failed to execute command: {}", e)));
        },
    }
}
