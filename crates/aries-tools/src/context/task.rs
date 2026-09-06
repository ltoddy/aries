use std::collections::{BinaryHeap, HashMap};
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use aries_event::Notifier;
use jiff::Timestamp;
use nix::sys::signal::{Signal, killpg};
use nix::unistd::Pid;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

const MAX_TASK_OUTPUT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Shell,
    Monitor,
}

impl Display for TaskKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskKind::Shell => write!(f, "shell"),
            TaskKind::Monitor => write!(f, "monitor"),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Running,
    Completed,
    Failed,
    Killed,
}

impl Display for TaskStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskStatus::Running => write!(f, "running"),
            TaskStatus::Completed => write!(f, "completed"),
            TaskStatus::Failed => write!(f, "failed"),
            TaskStatus::Killed => write!(f, "killed"),
        }
    }
}

struct TaskState {
    kind: TaskKind,
    status: TaskStatus,
    command: String,
    description: Option<String>,
    stdout: BoundedOutput,
    stderr: BoundedOutput,
    exit_code: Option<i32>,
    started_at: Timestamp,
    finished_at: Option<Timestamp>,
    pid: Option<u32>,
}

impl TaskState {
    fn new(
        kind: TaskKind,
        command: impl Into<String>,
        description: Option<String>,
        pid: Option<u32>,
    ) -> Self {
        Self {
            kind,
            status: TaskStatus::Running,
            command: command.into(),
            description,
            stdout: BoundedOutput::new(MAX_TASK_OUTPUT_BYTES),
            stderr: BoundedOutput::new(MAX_TASK_OUTPUT_BYTES),
            exit_code: None,
            started_at: Timestamp::now(),
            finished_at: None,
            pid,
        }
    }

    fn notification(&self, task_id: &str) -> String {
        let command = self.description.as_deref().unwrap_or(&self.command);
        let summary = match (self.kind, self.status) {
            (TaskKind::Monitor, TaskStatus::Completed) => {
                format!("Monitor \"{command}\" stream ended")
            },
            (TaskKind::Monitor, TaskStatus::Failed) => {
                format!("Monitor \"{command}\" script failed")
            },
            (TaskKind::Monitor, TaskStatus::Killed) => format!("Monitor \"{command}\" stopped"),
            (_, TaskStatus::Completed) => format!("Background command \"{command}\" completed"),
            (_, TaskStatus::Failed) => format!("Background command \"{command}\" failed"),
            (_, TaskStatus::Killed) => format!("Background command \"{command}\" was stopped"),
            (_, TaskStatus::Running) => return String::new(),
        };

        [
            "<task-notification>".to_owned(),
            format!("<task-id>{task_id}</task-id>"),
            format!("<task-kind>{:?}</task-kind>", self.kind),
            format!("<status>{:?}</status>", self.status),
            format!("<summary>{summary}</summary>"),
            "</task-notification>".to_owned(),
        ]
        .join("\n")
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskSnapshot {
    pub task_id: String,
    pub kind: TaskKind,
    pub status: TaskStatus,
    pub command: String,
    pub description: Option<String>,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub started_at: Timestamp,
    pub finished_at: Option<Timestamp>,
}

impl TaskSnapshot {
    fn new(task_id: impl Into<String>, state: &TaskState) -> Self {
        let task_id = task_id.into();

        Self {
            task_id,
            kind: state.kind,
            status: state.status,
            command: state.command.clone(),
            description: state.description.clone(),
            stdout: state.stdout.snapshot(),
            stderr: state.stderr.snapshot(),
            exit_code: state.exit_code,
            started_at: state.started_at,
            finished_at: state.finished_at,
        }
    }
}

#[derive(Clone)]
pub struct TaskRegistry {
    inner: Arc<Mutex<TaskRegistryInner>>,
    next_id: Arc<AtomicU64>,
    notifier: Notifier,
    changed: Arc<Notify>,
}

impl std::fmt::Debug for TaskRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskRegistry").finish_non_exhaustive()
    }
}

struct TaskRegistryInner {
    tasks: HashMap<String, Arc<Mutex<TaskState>>>,
}

impl TaskRegistryInner {
    fn new() -> Self {
        Self { tasks: HashMap::new() }
    }
}

impl TaskRegistry {
    pub fn new(notifier: Notifier) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TaskRegistryInner::new())),
            next_id: Arc::new(AtomicU64::new(0)),
            notifier,
            changed: Arc::new(Notify::new()),
        }
    }

    pub async fn spawn(
        &self,
        kind: TaskKind,
        cwd: impl AsRef<Path>,
        command: impl AsRef<str>,
        description: Option<String>,
    ) -> Result<TaskSnapshot, std::io::Error> {
        let command = command.as_ref();
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_owned());
        let mut child = Command::new(shell)
            .arg("-c")
            .arg(command)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let pid = child.id();
        let task_id = self.next_task_id(kind);
        let state = Arc::new(Mutex::new(TaskState::new(kind, command, description, pid)));

        {
            let mut registry = self.inner.lock();
            registry.tasks.insert(task_id.clone(), state.clone());
        }
        let stdout_reader = pipe_output(stdout, state.clone(), OutputStream::Stdout);
        let stderr_reader = pipe_output(stderr, state.clone(), OutputStream::Stderr);

        let task_id_for_waiter = task_id.clone();
        let state_for_waiter = state.clone();
        let notifier = Notifier::clone(&self.notifier);
        let changed = Arc::clone(&self.changed);
        tokio::spawn(async move {
            let result = child.wait().await;
            for reader in [stdout_reader, stderr_reader] {
                let _ = reader.await;
            }
            let mut state = state_for_waiter.lock();
            if state.status != TaskStatus::Running {
                return;
            }
            match result {
                Ok(status) => {
                    let exit_code = status.code().unwrap_or(-1);
                    state.exit_code = Some(exit_code);
                    state.status =
                        if exit_code == 0 { TaskStatus::Completed } else { TaskStatus::Failed };
                },
                Err(err) => {
                    state.stderr.push(&format!("\nfailed to wait for command: {err}"));
                    state.exit_code = Some(-1);
                    state.status = TaskStatus::Failed;
                },
            }
            state.finished_at = Some(Timestamp::now());
            notifier.notify(state.notification(&task_id_for_waiter));
            changed.notify_waiters();
        });

        Ok(TaskSnapshot::new(&task_id, &state.lock()))
    }

    pub fn get(&self, task_id: impl AsRef<str>) -> Option<TaskSnapshot> {
        let task_id = task_id.as_ref();
        let guard = self.inner.lock();
        let state = guard.tasks.get(task_id)?;
        Some(TaskSnapshot::new(task_id, &state.lock()))
    }

    pub async fn stop(&self, task_id: impl AsRef<str>) -> Result<TaskSnapshot, StopTaskError> {
        let task_id = task_id.as_ref();

        let state = {
            let guard = self.inner.lock();
            guard.tasks.get(task_id).cloned().ok_or(StopTaskError::NotFound)?
        };

        let pid = {
            let state = state.lock();
            if state.status != TaskStatus::Running {
                return Err(StopTaskError::NotRunning);
            }
            state.pid
        };

        if let Some(pid) = pid {
            killpg(Pid::from_raw(pid as i32), Signal::SIGTERM).map_err(StopTaskError::Signal)?;
        }

        let mut state = state.lock();
        state.status = TaskStatus::Killed;
        state.exit_code = Some(137);
        state.finished_at = Some(Timestamp::now());

        self.notifier.notify(state.notification(task_id));
        self.changed.notify_waiters();

        Ok(TaskSnapshot::new(task_id, &state))
    }

    fn next_task_id(&self, kind: TaskKind) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{kind}{id:08}")
    }

    pub async fn wait_for_change(&self) {
        self.changed.notified().await;
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StopTaskError {
    #[error("task not found")]
    NotFound,
    #[error("task is not running")]
    NotRunning,
    #[error("failed to stop task: {0}")]
    Io(std::io::Error),
    #[error("failed to signal task: {0}")]
    Signal(nix::Error),
}

enum OutputStream {
    Stdout,
    Stderr,
}

fn pipe_output(
    stream: Option<impl tokio::io::AsyncRead + Unpin + Send + 'static>,
    state: Arc<Mutex<TaskState>>,
    output: OutputStream,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let Some(stream) = stream else { return };
        let mut reader = BufReader::new(stream);
        let mut buffer = [0; 8192];
        loop {
            let bytes = match reader.read(&mut buffer).await {
                Ok(0) => return,
                Ok(bytes) => bytes,
                Err(_) => return,
            };
            let chunk = String::from_utf8_lossy(&buffer[..bytes]);
            let mut state = state.lock();
            match output {
                OutputStream::Stdout => state.stdout.push(chunk),
                OutputStream::Stderr => state.stderr.push(chunk),
            }
        }
    })
}

#[derive(Debug, Clone)]
struct BoundedOutput {
    max_bytes: usize,
    chunks: BinaryHeap<OutputChunk>,
    total_bytes: usize,
    next_seq: u64,
}

impl BoundedOutput {
    fn new(max_bytes: usize) -> Self {
        Self { max_bytes, chunks: BinaryHeap::new(), total_bytes: 0, next_seq: 0 }
    }

    fn push(&mut self, chunk: impl AsRef<str>) {
        let chunk = chunk.as_ref();

        if chunk.is_empty() {
            return;
        }

        let text = chunk.to_owned();
        self.total_bytes += text.len();
        self.chunks.push(OutputChunk { seq: self.next_seq, text });
        self.next_seq += 1;

        while self.total_bytes > self.max_bytes {
            let Some(oldest) = self.chunks.pop() else { break };
            self.total_bytes = self.total_bytes.saturating_sub(oldest.text.len());
        }
    }

    fn snapshot(&self) -> String {
        let mut chunks = self.chunks.clone().into_sorted_vec();
        chunks.reverse();
        chunks.into_iter().map(|chunk| chunk.text).collect()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct OutputChunk {
    seq: u64,
    text: String,
}

impl Ord for OutputChunk {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.seq.cmp(&self.seq)
    }
}

impl PartialOrd for OutputChunk {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundedOutput, TaskKind, TaskRegistry, TaskState, TaskStatus};
    use aries_event::Notifier;

    #[test]
    fn bounded_output_drops_oldest_chunks() {
        let mut output = BoundedOutput::new(5);
        output.push("ab");
        output.push("cd");
        output.push("ef");

        assert_eq!(output.snapshot(), "cdef");
    }

    #[test]
    fn bounded_output_ignores_empty_chunks() {
        let mut output = BoundedOutput::new(5);
        output.push("");
        output.push("ab");
        output.push("");

        assert_eq!(output.snapshot(), "ab");
    }

    #[test]
    fn bounded_output_preserves_insertion_order() {
        let mut output = BoundedOutput::new(16);
        output.push("ab");
        output.push("cd");
        output.push("ef");

        assert_eq!(output.snapshot(), "abcdef");
    }

    #[test]
    fn task_notification_uses_description_when_present() {
        let state = TaskState::new(
            TaskKind::Shell,
            "sleep 1",
            Some("run background sleep".to_owned()),
            None,
        );
        let state = TaskState { status: TaskStatus::Completed, ..state };

        let notification = state.notification("shell00000001");

        assert!(notification.contains("<task-id>shell00000001</task-id>"));
        assert!(notification.contains("<task-kind>Shell</task-kind>"));
        assert!(notification.contains("<status>Completed</status>"));
        assert!(notification.contains("Background command \"run background sleep\" completed"));
    }

    #[test]
    fn task_notification_uses_command_without_description() {
        let state = TaskState::new(TaskKind::Monitor, "tail -f log", None, None);
        let state = TaskState { status: TaskStatus::Killed, ..state };

        let notification = state.notification("monitor00000001");

        assert!(notification.contains("Monitor \"tail -f log\" stopped"));
    }

    #[test]
    fn running_task_has_no_notification() {
        let state = TaskState::new(TaskKind::Shell, "sleep 1", None, None);

        assert_eq!(state.notification("shell00000001"), "");
    }

    #[test]
    fn next_task_id_is_monotonic_per_registry() {
        let (notifier, _receiver) = Notifier::channel();
        let registry = TaskRegistry::new(notifier);

        assert_eq!(registry.next_task_id(TaskKind::Shell), "shell00000001");
        assert_eq!(registry.next_task_id(TaskKind::Monitor), "monitor00000002");
        assert_eq!(registry.next_task_id(TaskKind::Shell), "shell00000003");
    }
}
