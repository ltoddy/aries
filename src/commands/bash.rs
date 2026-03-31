use std::io::Write;
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

    println!("Executing: {}", command);

    let mut child =
        Command::new("bash").arg("-c").arg(command).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();

    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let stdout_handle = tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            match tokio::io::AsyncReadExt::read(&mut stdout, &mut buf).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let output = &buf[..n];
                    // Use standard library for writing to preserve colors
                    std::io::stdout().lock().write_all(output).unwrap();
                    std::io::stdout().lock().flush().unwrap();
                },
                Err(_) => break,
            }
        }
    });

    let stderr_handle = tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        loop {
            match tokio::io::AsyncReadExt::read(&mut stderr, &mut buf).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let output = &buf[..n];
                    std::io::stderr().lock().write_all(output).unwrap();
                    std::io::stderr().lock().flush().unwrap();
                },
                Err(_) => break,
            }
        }
    });

    let status = child.wait().await.unwrap();

    let _ = stdout_handle.await;
    let _ = stderr_handle.await;

    if !status.success() {
        let exit_code = status.code().unwrap_or(-1);
        eprintln!("Command exited with code: {}", exit_code.to_string().red());
    }
}
