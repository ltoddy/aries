use std::process::Stdio;

use bytes::BytesMut;
use colored::Colorize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::Command;

pub const NAME: &str = "/bash";

pub async fn execute(command: &str) {
    let command = command.trim();
    if command.is_empty() {
        eprintln!("{}", format!("No command provided after {NAME}").red());
        return;
    }

    let mut child =
        match Command::new("bash").arg("-c").arg(command).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
            Ok(child) => child,
            Err(_) => {
                eprintln!("{}", "Failed to spawn command".red());
                return;
            },
        };

    match (child.stdout.take(), child.stderr.take()) {
        (Some(stdout), Some(stderr)) => {
            let on_stdout = tokio::spawn(stream_output(stdout, tokio::io::stdout(), "stdout"));
            let on_stderr = tokio::spawn(stream_output(stderr, tokio::io::stderr(), "stderr"));

            let _ = tokio::try_join!(on_stdout, on_stderr);
        },
        (_, _) => eprintln!("{}", "Failed to capture stdout or stderr".red()),
    }
}

async fn stream_output<R, W>(mut reader: R, mut writer: W, stream_name: &'static str)
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = BytesMut::with_capacity(1024);
    loop {
        buf.clear();
        match reader.read_buf(&mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                let _ = writer.write_all(&buf).await;
                let _ = writer.flush().await;
            },
            Err(e) => {
                eprintln!("Error reading from {stream_name}: {e}");
                break;
            },
        }
    }
}
