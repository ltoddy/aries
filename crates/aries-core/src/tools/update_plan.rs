use anyhow::Result;
use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{RenderError, ToolArgsRender, ToolOutputRender};
use crate::event::AgentEvent;

pub const NAME: &str = "UpdatePlan";

const MAIN_AGENT_NAME: &str = "main";

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePlanArgs {
    pub items: Vec<PlanEntry>,
}

impl UpdatePlanArgs {
    pub fn title(&self) -> String {
        if self.items.is_empty() {
            "Clear plan".to_string()
        } else {
            format!("Update plan with {} items", self.items.len())
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PlanEntry {
    pub content: String,
    pub priority: PlanEntryPriority,
    pub status: PlanEntryStatus,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanEntryPriority {
    High,
    Medium,
    Low,
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
}

impl ToolArgsRender for UpdatePlanArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;

        let first = format!("{} plan entries", args.items.len());
        if args.items.is_empty() {
            return Ok((first, None));
        }

        let detail = args
            .items
            .into_iter()
            .map(|item| {
                let priority = match item.priority {
                    PlanEntryPriority::High => "high",
                    PlanEntryPriority::Medium => "medium",
                    PlanEntryPriority::Low => "low",
                };
                let status = match item.status {
                    PlanEntryStatus::Pending => "pending",
                    PlanEntryStatus::InProgress => "in_progress",
                    PlanEntryStatus::Completed => "completed",
                };
                format!("- [{}|{}] {}", priority, status, item.content)
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok((first, Some(detail)))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePlanOutput {
    pub ok: bool,
}

impl ToolOutputRender for UpdatePlanOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let _: Self = serde_json::from_str(raw)?;
        Ok("Plan updated.".to_string())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum UpdatePlanError {
    #[error("Failed to send plan update: receiver dropped")]
    SendFailed,
}

pub struct UpdatePlanTool {
    sender: UnboundedSender<AgentEvent>,
}

impl UpdatePlanTool {
    pub fn new(sender: UnboundedSender<AgentEvent>) -> Self {
        Self { sender }
    }
}

impl Tool for UpdatePlanTool {
    const NAME: &'static str = NAME;
    type Error = UpdatePlanError;
    type Args = UpdatePlanArgs;
    type Output = UpdatePlanOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/update_plan.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "items": {
                        "type": "array",
                        "description": "Structured plan entries. Pass an empty array to clear the plan.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": { "type": "string" },
                                "priority": { "type": "string", "enum": ["high", "medium", "low"] },
                                "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] }
                            },
                            "required": ["content", "priority", "status"]
                        }
                    }
                },
                "required": ["items"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let event = AgentEvent::from_plan(true, MAIN_AGENT_NAME, args.items);
        self.sender.send(event).map_err(|_| UpdatePlanError::SendFailed)?;
        Ok(UpdatePlanOutput { ok: true })
    }
}
