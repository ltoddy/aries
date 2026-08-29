use serde::{Deserialize, Serialize};

use crate::context::{TaskKind, TaskStatus};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaskStopOutput {
    pub task_id: String,
    pub task_type: TaskKind,
    pub status: TaskStatus,
    pub command: String,
}

impl TaskStopOutput {
    pub fn new(
        task_id: impl Into<String>,
        task_type: TaskKind,
        status: TaskStatus,
        command: impl Into<String>,
    ) -> Self {
        let task_id = task_id.into();
        let command = command.into();

        Self { task_id, task_type, status, command }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!(
            "Stopped background task {} ({:?}): {}",
            output.task_id, output.task_type, output.command
        ))
    }
}
