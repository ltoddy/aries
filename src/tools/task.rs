use anyhow::Result;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Deserialize)]
pub struct TaskArgs {
    description: String,
    prompt: String,
    subagent_type: String,
    task_id: Option<String>,
}

#[derive(Serialize)]
pub struct TaskOutput {
    task_id: String,
    result: String,
}

#[derive(thiserror::Error, Debug)]
pub enum TaskError {
    #[error("Task execution failed: {0}")]
    ExecutionError(String),
}

pub struct TaskTool;

impl Tool for TaskTool {
    const NAME: &'static str = "task";
    type Error = TaskError;
    type Args = TaskArgs;
    type Output = TaskOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        let mut desc = include_str!("descriptions/task.txt").to_string();
        // MVP: We only have the main agent for now
        desc = desc.replace("{agents}", "- default: The standard Aries agent with all standard tools");

        ToolDefinition {
            name: Self::NAME.to_string(),
            description: desc,
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "description": {
                        "type": "string",
                        "description": "A short (3-5 words) description of the task"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "The highly detailed task description for the agent to perform autonomously"
                    },
                    "subagent_type": {
                        "type": "string",
                        "description": "The type of agent to launch (e.g. 'default')"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Optional task ID to resume a previous subagent session"
                    }
                },
                "required": ["description", "prompt", "subagent_type"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let task_id = args.task_id.unwrap_or_else(|| Uuid::new_v4().to_string());

        // For MVP, we just return a placeholder.
        // A real implementation would spawn a new rig-core Agent instance,
        // give it the prompt, let it run until it finishes, and return its final
        // message. This requires significant state management and recursive
        // agent loops.

        Ok(TaskOutput {
            task_id,
            result: format!(
                "Task '{}' received. Subagent execution is a placeholder in this MVP. The prompt was: {}",
                args.description, args.prompt
            ),
        })
    }
}
