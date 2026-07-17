use serde::{Deserialize, Serialize};

use crate::update_plan::{PlanEntry, PlanEntryPriority, PlanEntryStatus};

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

impl UpdatePlanArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
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
