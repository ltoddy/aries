use std::process::Stdio;

use aries_theme::Theme;
use tokio::process::Command;

pub const NAME: &str = "/shell";

pub async fn execute(command: &str, theme: &Theme) {
    let command = command.trim();
    if command.is_empty() {
        eprintln!("{}", theme.red_text(&format!("No command provided after {NAME}")));
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
