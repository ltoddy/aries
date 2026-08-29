use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use aries_event::Notifier;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Bash,
    Monitor,
}

impl Display for TaskKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskKind::Bash => write!(f, "bash"),
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
    pub started_at_millis: u128,
    pub finished_at_millis: Option<u128>,
}

struct TaskState {
    kind: TaskKind,
    status: TaskStatus,
    command: String,
    description: Option<String>,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
    started_at_millis: u128,
    finished_at_millis: Option<u128>,
    pid: Option<u32>,
}

#[derive(Clone)]
pub struct TaskRegistry {
    inner: Arc<Mutex<TaskRegistryInner>>,
    next_id: Arc<AtomicU64>,
}

impl std::fmt::Debug for TaskRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskRegistry").finish_non_exhaustive()
    }
}

struct TaskRegistryInner {
    tasks: HashMap<String, Arc<Mutex<TaskState>>>,
    notifier: Notifier,
}

impl TaskRegistry {
    pub fn new(notifier: Notifier) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TaskRegistryInner { tasks: HashMap::new(), notifier })),
            next_id: Arc::new(AtomicU64::new(0)),
        }
    }

    pub async fn spawn(
        &self,
        kind: TaskKind,
        cwd: impl AsRef<Path>,
        command: String,
        description: Option<String>,
    ) -> Result<TaskSnapshot, std::io::Error> {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "bash".to_owned());
        let mut child = Command::new(shell)
            .arg("-c")
            .arg(&command)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let pid = child.id();
        let task_id = self.next_task_id(kind);
        let state = Arc::new(Mutex::new(TaskState {
            kind,
            status: TaskStatus::Running,
            command,
            description,
            stdout: String::new(),
            stderr: String::new(),
            exit_code: None,
            started_at_millis: now_millis(),
            finished_at_millis: None,
            pid,
        }));

        let notifier = {
            let mut registry = self.inner.lock();
            let notifier = registry.notifier.clone();
            registry.tasks.insert(task_id.clone(), state.clone());
            notifier
        };
        let stdout_reader = spawn_output_reader(stdout, state.clone(), OutputStream::Stdout);
        let stderr_reader = spawn_output_reader(stderr, state.clone(), OutputStream::Stderr);
        spawn_waiter(
            task_id.clone(),
            child,
            state.clone(),
            notifier,
            vec![stdout_reader, stderr_reader],
        );

        Ok(snapshot(&task_id, &state.lock()))
    }

    pub fn get(&self, task_id: &str) -> Option<TaskSnapshot> {
        let state = self.inner.lock().tasks.get(task_id)?.clone();
        Some(snapshot(task_id, &state.lock()))
    }

    pub async fn stop(&self, task_id: impl AsRef<str>) -> Result<TaskSnapshot, StopTaskError> {
        let task_id = task_id.as_ref();

        let state = self.inner.lock().tasks.get(task_id).cloned().ok_or(StopTaskError::NotFound)?;

        let pid = {
            let state = state.lock();
            if state.status != TaskStatus::Running {
                return Err(StopTaskError::NotRunning);
            }
            state.pid
        };

        if let Some(pid) = pid {
            Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status()
                .await
                .map_err(StopTaskError::Io)?;
        }

        let notifier = self.inner.lock().notifier.clone();
        let mut state = state.lock();
        state.status = TaskStatus::Killed;
        state.exit_code = Some(137);
        state.finished_at_millis = Some(now_millis());
        notifier.notify(notification(task_id, &state));
        Ok(snapshot(task_id, &state))
    }

    fn next_task_id(&self, kind: TaskKind) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{kind}{id:08}")
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
}

enum OutputStream {
    Stdout,
    Stderr,
}

fn spawn_output_reader(
    stream: Option<impl tokio::io::AsyncRead + Unpin + Send + 'static>,
    state: Arc<Mutex<TaskState>>,
    output_stream: OutputStream,
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
            match output_stream {
                OutputStream::Stdout => state.stdout.push_str(&chunk),
                OutputStream::Stderr => state.stderr.push_str(&chunk),
            }
        }
    })
}

fn spawn_waiter(
    task_id: String,
    mut child: tokio::process::Child,
    state: Arc<Mutex<TaskState>>,
    notifier: Notifier,
    output_readers: Vec<JoinHandle<()>>,
) {
    tokio::spawn(async move {
        let wait_result = child.wait().await;
        for reader in output_readers {
            let _ = reader.await;
        }
        let mut state = state.lock();
        if state.status != TaskStatus::Running {
            return;
        }
        match wait_result {
            Ok(status) => {
                let exit_code = status.code().unwrap_or(-1);
                state.exit_code = Some(exit_code);
                state.status =
                    if exit_code == 0 { TaskStatus::Completed } else { TaskStatus::Failed };
            },
            Err(err) => {
                state.stderr.push_str(&format!("\nfailed to wait for command: {err}"));
                state.exit_code = Some(-1);
                state.status = TaskStatus::Failed;
            },
        }
        state.finished_at_millis = Some(now_millis());
        notifier.notify(notification(&task_id, &state));
    });
}

fn notification(task_id: &str, state: &TaskState) -> String {
    let command = state.description.as_deref().unwrap_or(&state.command);
    let summary = match (state.kind, state.status) {
        (TaskKind::Monitor, TaskStatus::Completed) => format!("Monitor \"{command}\" stream ended"),
        (TaskKind::Monitor, TaskStatus::Failed) => format!("Monitor \"{command}\" script failed"),
        (TaskKind::Monitor, TaskStatus::Killed) => format!("Monitor \"{command}\" stopped"),
        (_, TaskStatus::Completed) => format!("Background command \"{command}\" completed"),
        (_, TaskStatus::Failed) => format!("Background command \"{command}\" failed"),
        (_, TaskStatus::Killed) => format!("Background command \"{command}\" was stopped"),
        (_, TaskStatus::Running) => return String::new(),
    };

    format!(
        "<task-notification>\n<task-id>{task_id}</task-id>\n<task-type>{:?}</task-type>\n<status>{:?}</status>\n<summary>{summary}</summary>\n</task-notification>",
        state.kind, state.status
    )
}

fn snapshot(task_id: &str, state: &TaskState) -> TaskSnapshot {
    TaskSnapshot {
        task_id: task_id.to_owned(),
        kind: state.kind,
        status: state.status,
        command: state.command.clone(),
        description: state.description.clone(),
        stdout: state.stdout.clone(),
        stderr: state.stderr.clone(),
        exit_code: state.exit_code,
        started_at_millis: state.started_at_millis,
        finished_at_millis: state.finished_at_millis,
    }
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}
