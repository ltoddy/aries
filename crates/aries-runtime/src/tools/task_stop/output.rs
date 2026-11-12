use serde::{Deserialize, Serialize};

use crate::tools::task::{TaskKind, TaskSnapshot, TaskStatus};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskStopOutput {
    pub task_id: String,
    pub task_type: TaskKind,
    pub status: TaskStatus,
    pub command: String,
}

impl TaskStopOutput {
    pub fn new(snapshot: TaskSnapshot) -> Self {
        Self {
            task_id: snapshot.task_id,
            task_type: snapshot.kind,
            status: snapshot.status,
            command: snapshot.command,
        }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!(
            "Background task {} ({:?}) is {}: {}",
            output.task_id, output.task_type, output.status, output.command
        ))
    }
}
