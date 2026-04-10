use std::marker::PhantomData;

use anyhow::Result;
use aries_config::AriesConfig;
use futures::StreamExt;
use rig::agent::PromptHook;
use rig::completion;
use rig::completion::ToolDefinition;
use rig::providers::{azure, openai};
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::AgentWrapper;
use crate::agent_type::AgentType;

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskArgs {
    pub description: String,
    pub prompt: String,
    pub subagent_type: String,
    pub task_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskOutput {
    pub task_id: String,
    pub result: String,
}

#[derive(thiserror::Error, Debug)]
pub enum TaskError {
    #[error("Task execution failed: {0}")]
    ExecutionError(String),
}

pub struct TaskTool<M, P = ()>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    config: AriesConfig,
    hook: P,
    _phantom: PhantomData<M>,
}

impl<M, P> TaskTool<M, P>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    pub fn new(config: AriesConfig, hook: P) -> Self {
        Self { config, hook, _phantom: Default::default() }
    }
}

impl<P> Tool for TaskTool<openai::CompletionModel, P>
where
    P: PromptHook<openai::CompletionModel> + 'static,
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
            "default" | "build" => AgentType::Build,
            "explore" => AgentType::Explore,
            "plan" => AgentType::Plan,
            "general" => AgentType::General,
            _ => AgentType::General,
        };

        let name = format!("Subagent [{}]", args.subagent_type);

        let mut agent =
            AgentWrapper::<openai::CompletionModel, P>::new(name, self.config.clone(), agent_type, self.hook.clone())
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

impl<P> Tool for TaskTool<azure::CompletionModel, P>
where
    P: PromptHook<azure::CompletionModel> + 'static,
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
            "default" | "build" => AgentType::Build,
            "explore" => AgentType::Explore,
            "plan" => AgentType::Plan,
            "general" => AgentType::General,
            _ => AgentType::General,
        };

        let name = format!("Subagent [{}]", args.subagent_type);

        let mut agent =
            AgentWrapper::<azure::CompletionModel, P>::new(name, self.config.clone(), agent_type, self.hook.clone())
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
