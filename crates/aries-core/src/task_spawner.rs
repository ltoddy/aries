use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::process::Command;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct TaskNotification {
    pub task_id: String,
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Completed,
}

#[derive(Debug, Clone)]
pub struct TaskInfo {
    pub command: String,
    pub status: TaskStatus,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Clone)]
pub struct TaskSpawner {
    tasks: Arc<Mutex<HashMap<String, TaskInfo>>>,
    tx: mpsc::UnboundedSender<TaskNotification>,
}

#[derive(Clone)]
pub struct NotificationReceiver {
    rx: Arc<Mutex<mpsc::UnboundedReceiver<TaskNotification>>>,
}

impl TaskSpawner {
    pub fn new() -> (Self, NotificationReceiver) {
        let (tx, rx) = mpsc::unbounded_channel();
        let manager = Self { tasks: Arc::new(Mutex::new(HashMap::new())), tx };
        let receiver = NotificationReceiver { rx: Arc::new(Mutex::new(rx)) };
        (manager, receiver)
    }

    pub fn noop() -> Self {
        let (tx, _) = mpsc::unbounded_channel();
        Self { tasks: Arc::new(Mutex::new(HashMap::new())), tx }
    }

    pub fn run(&self, command: String) -> String {
        let task_id = nanoid::nanoid!(8);

        {
            let mut tasks = self.tasks.lock();
            tasks.insert(
                task_id.clone(),
                TaskInfo {
                    command: command.clone(),
                    status: TaskStatus::Running,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                },
            );
        }

        let manager = self.clone();
        let tid = task_id.clone();
        tokio::spawn(async move {
            manager.execute(tid, command).await;
        });

        task_id
    }

    pub fn check(&self, task_id: &str) -> Option<TaskInfo> {
        let tasks = self.tasks.lock();
        tasks.get(task_id).cloned()
    }

    async fn execute(&self, task_id: String, command: String) {
        let result = Command::new("sh")
            .arg("-c")
            .arg(&command)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await;

        let (stdout, stderr, exit_code) = match result {
            Ok(output) => {
                let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();
                truncate_output(&mut stdout, 50000);
                truncate_output(&mut stderr, 50000);
                let exit_code = output.status.code().unwrap_or(-1);
                (stdout, stderr, exit_code)
            },
            Err(e) => (String::new(), format!("Error: {}", e), -1),
        };

        {
            let mut tasks = self.tasks.lock();
            if let Some(info) = tasks.get_mut(&task_id) {
                info.status = TaskStatus::Completed;
                info.stdout = Some(stdout.clone());
                info.stderr = Some(stderr.clone());
                info.exit_code = Some(exit_code);
            }
        }

        let _ = self.tx.send(TaskNotification {
            task_id,
            command,
            stdout: truncate_str(&stdout, 500),
            stderr: truncate_str(&stderr, 500),
            exit_code,
        });
    }
}

impl NotificationReceiver {
    pub fn drain(&mut self) -> Vec<TaskNotification> {
        let mut notifications = Vec::new();
        while let Ok(notification) = self.rx.lock().try_recv() {
            notifications.push(notification);
        }
        notifications
    }
}

fn truncate_output(s: &mut String, max_bytes: usize) {
    if s.len() > max_bytes {
        let mut end = max_bytes;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        s.truncate(end);
        s.push_str("\n... (truncated)");
    }
}

fn truncate_str(s: &str, max_bytes: usize) -> String {
    if s.len() > max_bytes {
        let mut end = max_bytes;
        while !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}... (truncated)", &s[..end])
    } else {
        s.to_string()
    }
}
