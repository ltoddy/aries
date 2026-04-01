use std::io::{Read, Write};
use std::sync::Arc;

use colored::Colorize;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use tokio::sync::{Mutex, OnceCell, mpsc};

pub const NAME: &str = "/bash";

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

    let command_with_newline = format!("{}\n", command);
    if let Err(e) = session.writer.write_all(command_with_newline.as_bytes()) {
        eprintln!("{}", format!("Failed to write to bash session: {}", e).red());
        return;
    }

    wait_for_marker(&mut session, false).await;
}

const PROMPT_MARKER: &str = "ARIES_DONE_MARKER_8F3A2B1C";

struct Session {
    writer: Box<dyn Write + Send>,
    output_rx: mpsc::Receiver<Vec<u8>>,
}

static SESSION: OnceCell<Option<Arc<Mutex<Session>>>> = OnceCell::const_new();

async fn get_session() -> Option<Arc<Mutex<Session>>> {
    let session_opt = SESSION
        .get_or_init(|| async {
            let pty_system = native_pty_system();
            let pair = match pty_system.openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 }) {
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

            let mut session = Session { writer, output_rx: rx };

            let setup_cmd = b"stty -echo; export PS1=ARIES_DONE_MARKER_\"8F3A2B1C\"\n";
            if let Err(e) = session.writer.write_all(setup_cmd) {
                eprintln!("{}", format!("Failed to setup pty session: {}", e).red());
                return None;
            }

            wait_for_marker(&mut session, true).await;

            Some(Arc::new(Mutex::new(session)))
        })
        .await;
    session_opt.clone()
}

async fn wait_for_marker(session: &mut Session, is_setup: bool) {
    let mut buffer = String::with_capacity(1024);
    while let Some(chunk) = session.output_rx.recv().await {
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        if let Some(idx) = buffer.find(PROMPT_MARKER) {
            if !is_setup {
                print!("{}", &buffer[..idx]);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            }
            break;
        } else {
            if !is_setup {
                let safe_len = if buffer.len() > PROMPT_MARKER.len() { buffer.len() - PROMPT_MARKER.len() } else { 0 };
                if safe_len > 0 {
                    print!("{}", &buffer[..safe_len]);
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                    let remaining = buffer[safe_len..].to_string();
                    buffer = remaining;
                }
            }
        }
    }
}
