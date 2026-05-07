use std::fmt::{self, Display};

use anyhow::Result;
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use futures::StreamExt;
use rig::client::CompletionClient;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::agents::{AgentBuilder, AgentType};

pub const NAME: &str = "task";

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

impl Display for TaskOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.result)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TaskError {
    #[error("Task execution failed: {0}")]
    ExecutionError(String),
}

pub struct TaskTool<C>
where
    C: CompletionClient,
{
    client: C,
    config: AriesConfig,
    gctx: GlobalContext,
}

impl<C> TaskTool<C>
where
    C: CompletionClient,
{
    pub fn new(client: C, config: AriesConfig, gctx: GlobalContext) -> Self {
        Self { client, config, gctx }
    }
}

impl<C> Tool for TaskTool<C>
where
    C: CompletionClient + Clone + Send + Sync + 'static,
{
    const NAME: &'static str = NAME;
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

        let mut agent = AgentBuilder::new(
            self.client.clone(),
            self.config.clone(),
            agent_type,
            self.gctx.clone(),
        )
        .build();

        // TODO: hook
        let stream = agent.stream_prompt(&args.prompt, &[], ()).await;
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
