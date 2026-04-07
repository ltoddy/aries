use anyhow::Result;
use aries_config::AriesConfig;
use futures::StreamExt;
use rig::agent::PromptHook;
use rig::completion::ToolDefinition;
use rig::providers::openai;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::AgentWrapper;
use crate::agent_type::AgentType;

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

pub struct TaskTool<H = ()> {
    pub config: AriesConfig,
    pub hook: H,
}

impl<H> TaskTool<H> {
    pub fn new(config: AriesConfig, hook: H) -> Self {
        Self { config, hook }
    }
}

impl<H> Tool for TaskTool<H>
where
    H: PromptHook<openai::CompletionModel> + Clone + 'static,
{
    const NAME: &'static str = "task";
    type Error = TaskError;
    type Args = TaskArgs;
    type Output = TaskOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/task.txt").to_string(),
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
        let task_id = args.task_id.unwrap_or_else(|| nanoid::nanoid!());

        let agent_type = match args.subagent_type.as_str() {
            "explore" => AgentType::Explore,
            "plan" => AgentType::Plan,
            _ => AgentType::General,
        };

        let name = format!("Subagent [{}]", args.subagent_type);

        let mut agent = AgentWrapper::new(name, self.config.clone(), agent_type, self.hook.clone())
            .map_err(|e| TaskError::ExecutionError(format!("Failed to create agent: {}", e)))?;

        let stream = agent.stream_prompt(&args.prompt, &[]).await;
        tokio::pin!(stream);
        let mut final_res = rig::agent::FinalResponse::empty();

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(rig::agent::MultiTurnStreamItem::FinalResponse(res)) => final_res = res,
                Err(e) => return Err(TaskError::ExecutionError(format!("Subagent failed: {}", e))),
                Ok(_) => {},
            }
        }

        let res = final_res.response();
        Ok(TaskOutput { task_id, result: res.to_owned() })
    }
}
