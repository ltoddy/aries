use std::io::{Read, Write};
use std::sync::Arc;

use colored::Colorize;
use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use tokio::io::AsyncReadExt;
use tokio::sync::{Mutex, OnceCell, mpsc};

pub const NAME: &str = "/bash";

struct RawModeGuard;
impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

pub async fn execute(command: &str) {
    let command = command.trim();
    if command.is_empty() {
        eprintln!("{}", format!("No command provided after {NAME}").red());
        return;
    }

    let session = match get_session().await {
        Some(s) => s,
        None => return,
    };
    let mut session = session.lock().await;

    let _guard = {
        let _ = crossterm::terminal::enable_raw_mode();
        if let Ok((cols, rows)) = crossterm::terminal::size() {
            let _ = session.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
        }
        RawModeGuard
    };

    let command_with_newline = format!("{}\n", command);
    if let Err(e) = session.writer.write_all(command_with_newline.as_bytes()) {
        eprintln!("{}", format!("Failed to write to bash session: {}", e).red());
        return;
    }

    let mut stdin = tokio::io::stdin();
    let mut buf = [0u8; 1024];

    let Session { output_rx, writer, .. } = &mut *session;

    let mut buffer: Vec<u8> = Vec::with_capacity(1024);
    let marker_bytes = PROMPT_MARKER.as_bytes();

    loop {
        tokio::select! {
            // Process incoming output
            chunk_opt = output_rx.recv() => {
                if let Some(chunk) = chunk_opt {
                    buffer.extend_from_slice(&chunk);

                    // Check if we hit the done marker
                    if let Some(idx) = buffer.windows(marker_bytes.len()).position(|window| window == marker_bytes) {
                        use std::io::Write;
                        let mut stdout = std::io::stdout();
                        let _ = stdout.write_all(&buffer[..idx]);
                        let _ = stdout.flush();
                        break;
                    } else {
                        // Print what we safely can, keeping marker_bytes.len() buffered just in case
                        let safe_len = if buffer.len() > marker_bytes.len() { buffer.len() - marker_bytes.len() } else { 0 };
                        if safe_len > 0 {
                            use std::io::Write;
                            let mut stdout = std::io::stdout();
                            let _ = stdout.write_all(&buffer[..safe_len]);
                            let _ = stdout.flush();

                            let remaining = buffer[safe_len..].to_vec();
                            buffer = remaining;
                        }
                    }
                } else {
                    // Channel closed
                    break;
                }
            }
            // Forward input to PTY
            n_res = stdin.read(&mut buf) => {
                if let Ok(n) = n_res {
                    if n == 0 { break; }
                    let _ = writer.write_all(&buf[..n]);
                } else {
                    break;
                }
            }
        }
    }
}

const PROMPT_MARKER: &str = "ARIES_DONE_MARKER_8F3A2B1C";

struct Session {
    writer: Box<dyn Write + Send>,
    output_rx: mpsc::Receiver<Vec<u8>>,
    master: Box<dyn MasterPty + Send>,
}

static SESSION: OnceCell<Option<Arc<Mutex<Session>>>> = OnceCell::const_new();

async fn get_session() -> Option<Arc<Mutex<Session>>> {
    let session_opt = SESSION
        .get_or_init(|| async {
            let pty_system = native_pty_system();

            let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
            let pair = match pty_system.openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 }) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("{}", format!("Failed to open pty: {}", e).red());
                    return None;
                },
            };

            let mut cmd = CommandBuilder::new("bash");
            cmd.args(["--noprofile", "--norc"]);
            if let Ok(cwd) = std::env::current_dir() {
                cmd.cwd(cwd);
            }

            let _ = pair.slave.spawn_command(cmd);

            let (writer, mut reader) = match (pair.master.take_writer(), pair.master.try_clone_reader()) {
                (Ok(writer), Ok(reader)) => (writer, reader),
                (_, _) => {
                    eprintln!("{}", "Failed to take writer or clone reader".red());
                    return None;
                },
            };

            let (tx, rx) = mpsc::channel(1024);

            tokio::task::spawn_blocking(move || {
                let mut buf = [0u8; 1024];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            if tx.blocking_send(buf[..n].to_vec()).is_err() {
                                break;
                            }
                        },
                        Err(_) => break,
                    }
                }
            });

            let mut session = Session { writer, output_rx: rx, master: pair.master };

            let setup_cmd = b"stty raw -echo; export PS1=ARIES_DONE_MARKER_\"8F3A2B1C\"\n";
            if let Err(e) = session.writer.write_all(setup_cmd) {
                eprintln!("{}", format!("Failed to setup pty session: {}", e).red());
                return None;
            }

            wait_for_marker(&mut session.output_rx, true).await;

            Some(Arc::new(Mutex::new(session)))
        })
        .await;
    session_opt.clone()
}

async fn wait_for_marker(rx: &mut mpsc::Receiver<Vec<u8>>, is_setup: bool) {
    let mut buffer: Vec<u8> = Vec::with_capacity(1024);
    let marker_bytes = PROMPT_MARKER.as_bytes();

    while let Some(chunk) = rx.recv().await {
        buffer.extend_from_slice(&chunk);

        if let Some(idx) = buffer.windows(marker_bytes.len()).position(|window| window == marker_bytes) {
            if !is_setup {
                use std::io::Write;
                let mut stdout = std::io::stdout();
                let _ = stdout.write_all(&buffer[..idx]);
                let _ = stdout.flush();
            }
            break;
        } else {
            if !is_setup {
                let safe_len = if buffer.len() > marker_bytes.len() { buffer.len() - marker_bytes.len() } else { 0 };
                if safe_len > 0 {
                    use std::io::Write;
                    let mut stdout = std::io::stdout();
                    let _ = stdout.write_all(&buffer[..safe_len]);
                    let _ = stdout.flush();

                    // Keep the remaining bytes that might be part of the marker
                    let remaining = buffer[safe_len..].to_vec();
                    buffer = remaining;
                }
            }
        }
    }
}
