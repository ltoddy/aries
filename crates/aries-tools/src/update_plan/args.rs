use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolArgsRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePlanArgs {
    pub items: Vec<PlanEntry>,
}

impl UpdatePlanArgs {
    pub fn title(&self) -> String {
        if self.items.is_empty() {
            "Clear plan".to_owned()
        } else {
            format!("Update plan with {} items", self.items.len())
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanEntry {
    pub content: String,
    pub priority: PlanEntryPriority,
    pub status: PlanEntryStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanEntryPriority {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
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
