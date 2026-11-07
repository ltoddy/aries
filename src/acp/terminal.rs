use std::collections::HashMap;
use std::io;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use agent_client_protocol::Error;
use agent_client_protocol::v1::schema::{
    CreateTerminalRequest, SessionId, TerminalExitStatus, TerminalId, TerminalOutputResponse,
};
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, watch};

#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    #[error("终端工作目录必须为绝对路径")]
    InvalidCwd,
    #[error("{0}")]
    Spawn(#[source] io::Error),
    #[error("终端不存在或已释放")]
    NotFound,
    #[error("终端连接已关闭")]
    Closed(#[from] watch::error::RecvError),
}

impl From<TerminalError> for Error {
    fn from(error: TerminalError) -> Self {
        match error {
            TerminalError::InvalidCwd | TerminalError::NotFound => {
                Self::invalid_params().data(error.to_string())
            }
            TerminalError::Spawn(_) | TerminalError::Closed(_) => {
                Self::internal_error().data(error.to_string())
            }
        }
    }
}

#[derive(Debug)]
struct Terminal {
    output: watch::Receiver<TerminalOutputResponse>,
    kill: mpsc::UnboundedSender<()>,
}

#[derive(Debug)]
pub struct Terminals {
    entries: Mutex<HashMap<SessionId, HashMap<TerminalId, Terminal>>>,
    next_id: AtomicU64,
    prefix: u128,
}

impl Terminals {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            prefix: SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(),
        }
    }

    pub fn create(&self, request: CreateTerminalRequest) -> Result<TerminalId, TerminalError> {
        if request.cwd.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err(TerminalError::InvalidCwd);
        }
        let mut command = Command::new(&request.command);
        command
            .args(&request.args)
            .envs(request.env.iter().map(|env| (&env.name, &env.value)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &request.cwd {
            command.current_dir(cwd);
        }
        #[cfg(unix)]
        command.process_group(0);
        let child = command.spawn().map_err(TerminalError::Spawn)?;
        let process = Process { group: child.id(), child };
        let id = TerminalId::new(format!(
            "orbit-terminal-{}-{}",
            self.prefix,
            self.next_id.fetch_add(1, Ordering::Relaxed)
        ));
        let (output, receiver) = watch::channel(TerminalOutputResponse::new("", false));
        let (kill, kills) = mpsc::unbounded_channel();
        self.entries
            .lock()
            .expect("terminal entries lock poisoned")
            .entry(request.session_id)
            .or_default()
            .insert(id.clone(), Terminal { output: receiver, kill });
        let limit =
            usize::try_from(request.output_byte_limit.unwrap_or(1024 * 1024)).unwrap_or(usize::MAX);
        tokio::spawn(run(process, output, kills, limit));
        Ok(id)
    }

    pub fn output(
        &self,
        session: &SessionId,
        id: &TerminalId,
    ) -> Result<TerminalOutputResponse, TerminalError> {
        let entries = self.entries.lock().expect("terminal entries lock poisoned");
        entries
            .get(session)
            .and_then(|terminals| terminals.get(id))
            .map(|terminal| terminal.output.borrow().clone())
            .ok_or(TerminalError::NotFound)
    }

    fn receiver(
        &self,
        session: &SessionId,
        id: &TerminalId,
    ) -> Result<watch::Receiver<TerminalOutputResponse>, TerminalError> {
        self.entries
            .lock()
            .expect("terminal entries lock poisoned")
            .get(session)
            .and_then(|terminals| terminals.get(id))
            .map(|terminal| terminal.output.clone())
            .ok_or(TerminalError::NotFound)
    }

    pub async fn wait(
        &self,
        session: &SessionId,
        id: &TerminalId,
    ) -> Result<TerminalExitStatus, TerminalError> {
        let mut receiver = self.receiver(session, id)?;
        loop {
            if let Some(status) = receiver.borrow().exit_status.clone() {
                return Ok(status);
            }
            receiver.changed().await?;
        }
    }

    pub async fn kill(&self, session: &SessionId, id: &TerminalId) -> Result<(), TerminalError> {
        {
            let entries = self.entries.lock().expect("terminal entries lock poisoned");
            let terminal = entries
                .get(session)
                .and_then(|terminals| terminals.get(id))
                .ok_or(TerminalError::NotFound)?;
            let _ = terminal.kill.send(());
        }
        self.wait(session, id).await?;
        Ok(())
    }

    pub async fn release(&self, session: &SessionId, id: &TerminalId) -> Result<(), TerminalError> {
        self.kill(session, id).await?;
        let mut entries = self.entries.lock().expect("terminal entries lock poisoned");
        if let Some(terminals) = entries.get_mut(session) {
            terminals.remove(id);
            if terminals.is_empty() {
                entries.remove(session);
            }
        }
        Ok(())
    }
}

struct Process {
    child: Child,
    group: Option<u32>,
}

impl Process {
    fn kill(&mut self) {
        #[cfg(unix)]
        if let Some(group) = self.group {
            unsafe {
                libc::kill(-(group as i32), libc::SIGKILL);
            }
        }
        let _ = self.child.start_kill();
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.kill();
    }
}

fn append(
    output: &watch::Sender<TerminalOutputResponse>,
    pending: &mut Vec<u8>,
    bytes: &[u8],
    eof: bool,
    limit: usize,
) {
    pending.extend_from_slice(bytes);
    let mut complete = 0;
    while complete < pending.len() {
        match std::str::from_utf8(&pending[complete..]) {
            Ok(_) => {
                complete = pending.len();
                break;
            }
            Err(error) => {
                complete += error.valid_up_to();
                match error.error_len() {
                    Some(length) => complete += length,
                    None => {
                        if eof {
                            complete = pending.len();
                        }
                        break;
                    }
                }
            }
        }
    }
    if complete == 0 {
        return;
    }
    let text = String::from_utf8_lossy(&pending[..complete]);
    output.send_modify(|state| {
        state.output.push_str(&text);
        if state.output.len() > limit {
            let mut start = state.output.len() - limit;
            while !state.output.is_char_boundary(start) {
                start += 1;
            }
            state.output.drain(..start);
            state.truncated = true;
        }
    });
    pending.drain(..complete);
}

async fn run(
    mut process: Process,
    output: watch::Sender<TerminalOutputResponse>,
    mut kills: mpsc::UnboundedReceiver<()>,
    limit: usize,
) {
    let mut stdout = process.child.stdout.take().expect("stdout is piped");
    let mut stderr = process.child.stderr.take().expect("stderr is piped");
    let (mut out_bytes, mut err_bytes) = ([0; 4096], [0; 4096]);
    let (mut out_pending, mut err_pending) = (Vec::new(), Vec::new());
    let (mut out_open, mut err_open, mut killed) = (true, true, false);
    let mut status = None;
    let mut exit_deadline = None;
    loop {
        if status.is_some() && !out_open && !err_open {
            break;
        }
        tokio::select! {
            _ = async {
                if let Some(deadline) = exit_deadline {
                    tokio::time::sleep_until(deadline).await;
                }
            }, if exit_deadline.is_some() => break,
            _ = kills.recv(), if !killed => { process.kill(); killed = true; }
            result = process.child.wait(), if status.is_none() => {
                status = Some(match result {
                    Ok(exit) => {
                        let mut status = TerminalExitStatus::new().exit_code(exit.code().and_then(|code| u32::try_from(code).ok()));
                        #[cfg(unix)]
                        { use std::os::unix::process::ExitStatusExt; status.signal = exit.signal().map(|signal| signal.to_string()); }
                        status
                    }
                    Err(error) => {
                        append(&output, &mut out_pending, error.to_string().as_bytes(), true, limit);
                        TerminalExitStatus::new().signal("wait_failed")
                    }
                });
                exit_deadline = Some(tokio::time::Instant::now() + Duration::from_secs(1));
            }
            result = stdout.read(&mut out_bytes), if out_open => {
                let count = result.unwrap_or_default();
                out_open = count != 0;
                append(&output, &mut out_pending, &out_bytes[..count], !out_open, limit);
            }
            result = stderr.read(&mut err_bytes), if err_open => {
                let count = result.unwrap_or_default();
                err_open = count != 0;
                append(&output, &mut err_pending, &err_bytes[..count], !err_open, limit);
            }
        }
    }
    append(&output, &mut out_pending, &[], true, limit);
    append(&output, &mut err_pending, &[], true, limit);
    output.send_modify(|state| state.exit_status = status);
}

pub type SharedTerminals = Arc<Terminals>;
