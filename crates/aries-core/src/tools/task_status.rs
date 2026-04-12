use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::task_spawner::{TaskSpawner, TaskStatus};

pub const NAME: &str = "task_status";

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskStatusArgs {
    pub task_id: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskStatusOutput {
    pub task_id: String,
    pub status: String,
    pub command: String,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(thiserror::Error, Debug)]
pub enum TaskStatusError {
    #[error("{0}")]
    NotFound(String),
}

pub struct TaskStatusTool {
    spawner: TaskSpawner,
}

impl TaskStatusTool {
    pub fn new(spawner: TaskSpawner) -> Self {
        Self { spawner }
    }
}

impl Tool for TaskStatusTool {
    const NAME: &'static str = NAME;
    type Error = TaskStatusError;
    type Args = TaskStatusArgs;
    type Output = TaskStatusOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/task_status.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "task_id": {
                        "type": "string",
                        "description": "The task ID returned by task_spawn"
                    }
                },
                "required": ["task_id"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let info = self
            .spawner
            .check(&args.task_id)
            .ok_or_else(|| TaskStatusError::NotFound(format!("Task '{}' not found", args.task_id)))?;

        let status = match info.status {
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
        };

        Ok(TaskStatusOutput {
            task_id: args.task_id,
            status: status.to_string(),
            command: info.command,
            stdout: info.stdout,
            stderr: info.stderr,
            exit_code: info.exit_code,
        })
    }
}
