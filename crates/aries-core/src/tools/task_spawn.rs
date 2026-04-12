use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::task_spawner::TaskSpawner;

pub const NAME: &str = "task_spawn";

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskSpawnArgs {
    pub command: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskSpawnOutput {
    pub task_id: String,
    pub status: String,
}

#[derive(thiserror::Error, Debug)]
pub enum TaskSpawnError {
    #[error("{0}")]
    Failed(String),
}

pub struct TaskSpawnTool {
    spawner: TaskSpawner,
}

impl TaskSpawnTool {
    pub fn new(spawner: TaskSpawner) -> Self {
        Self { spawner }
    }
}

impl Tool for TaskSpawnTool {
    const NAME: &'static str = NAME;
    type Error = TaskSpawnError;
    type Args = TaskSpawnArgs;
    type Output = TaskSpawnOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/task_spawn.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to run in the background"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let task_id = self.spawner.run(args.command);
        Ok(TaskSpawnOutput { task_id, status: "started".to_string() })
    }
}
