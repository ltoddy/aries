use anyhow::Result;
use rig::client::ProviderClient;
use rig::completion::{Prompt, ToolDefinition};
use rig::providers::deepseek;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentType;

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct TaskArgs {
    description: String,
    prompt: String,
    subagent_type: String,
    task_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct TaskOutput {
    pub task_id: String,
    pub result: String,
}

#[derive(thiserror::Error, Debug)]
#[allow(dead_code)]
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
        // Support specific agents
        desc = desc.replace(
            "{agents}",
            "- default/build: The standard Aries agent with all standard tools\n\
             - explore: Fast agent specialized for exploring codebases\n\
             - plan: Planning agent that can read but not edit",
        );

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
                        "description": "The type of agent to launch (e.g. 'explore', 'plan', 'default')"
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

        // Execute the subagent logic here by spinning up a new agent instance.
        // For a full implementation, we need to pass down the API key and state.
        // Since Tool trait doesn't easily capture external references without state,
        // we'll fetch the client directly here for the MVP subagent execution.
        let client = deepseek::Client::from_env();

        let agent_type = match args.subagent_type.as_str() {
            "explore" => AgentType::Explore,
            "plan" => AgentType::Plan,
            _ => AgentType::General, // fallback to general
        };

        // In a real application, we would stream the output or manage nested
        // conversation loops
        let agent = agent_type.build_agent(&client, deepseek::DEEPSEEK_CHAT);

        // Execute sub-agent synchronously (or rather, await its completion)
        let response = agent
            .prompt(&args.prompt)
            .await
            .map_err(|e| TaskError::ExecutionError(format!("Subagent failed: {}", e)))?;

        Ok(TaskOutput { task_id, result: response })
    }
}
