use std::process::Stdio;

use tokio::process::Command;

use crate::theme::Theme;

pub const NAME: &str = "/bash";

pub async fn execute(command: &str, theme: &Theme) {
    let command = command.trim();
    if command.is_empty() {
        eprintln!("{}", theme.red_text(&format!("No command provided after {NAME}")));
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
                eprintln!("{}", theme.red_text(&format!("Command failed with status: {}", status)));
            }
        },
        Err(e) => {
            eprintln!("{}", theme.red_text(&format!("Failed to execute command: {}", e)));
        },
    }
}
