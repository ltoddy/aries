use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use aries_event::Notifier;
use jiff::Timestamp;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::task::JoinHandle;

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
    stdout: String,
    stderr: String,
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
            stdout: String::new(),
            stderr: String::new(),
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
            stdout: state.stdout.clone(),
            stderr: state.stderr.clone(),
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
                    state.stderr.push_str(&format!("\nfailed to wait for command: {err}"));
                    state.exit_code = Some(-1);
                    state.status = TaskStatus::Failed;
                },
            }
            state.finished_at = Some(Timestamp::now());
            notifier.notify(state.notification(&task_id_for_waiter));
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
            Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status()
                .await
                .map_err(StopTaskError::Io)?;
        }

        let mut state = state.lock();
        state.status = TaskStatus::Killed;
        state.exit_code = Some(137);
        state.finished_at = Some(Timestamp::now());

        self.notifier.notify(state.notification(task_id));

        Ok(TaskSnapshot::new(task_id, &state))
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
                OutputStream::Stdout => state.stdout.push_str(&chunk),
                OutputStream::Stderr => state.stderr.push_str(&chunk),
            }
        }
    })
}
