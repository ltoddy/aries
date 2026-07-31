mod args;
mod entry;
mod error;
mod output;
#[cfg(test)]
mod tests;

use rig_agent::tool::{Tool, ToolContext};
use serde_json::Value;

pub use self::args::UpdatePlanArgs;
pub use self::entry::{PlanEntry, PlanEntryPriority, PlanEntryStatus};
pub use self::error::UpdatePlanError;
pub use self::output::UpdatePlanOutput;

pub const NAME: &str = "UpdatePlan";

pub struct UpdatePlanTool {}

impl Default for UpdatePlanTool {
    fn default() -> Self {
        Self::new()
    }
}

impl UpdatePlanTool {
    pub fn new() -> Self {
        Self {}
    }
}

impl Tool for UpdatePlanTool {
    const NAME: &'static str = NAME;
    type Error = UpdatePlanError;
    type Args = UpdatePlanArgs;
    type Output = UpdatePlanOutput;

    fn description(&self) -> String {
        include_str!("description.md").to_owned()
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "items": {
                    "type": "array",
                    "description": "Structured plan entries. Pass an empty array to clear the plan.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "content": { "type": "string", "description": "Imperative form of the task (e.g. \"Run tests\")" },
                            "active_form": { "type": "string", "description": "Present continuous form shown while executing (e.g. \"Running tests\")" },
                            "priority": { "type": "string", "enum": ["high", "medium", "low"] },
                            "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] }
                        },
                        "required": ["content", "active_form", "priority", "status"]
                    }
                }
            },
            "required": ["items"]
        })
    }

    async fn call(
        &self,
        _context: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        if args.items.iter().any(|v| v.content.trim().is_empty()) {
            return Err(UpdatePlanError::EmptyContent);
        }
        if args.items.iter().any(|v| v.active_form.trim().is_empty()) {
            return Err(UpdatePlanError::EmptyActiveForm);
        }

        let all_done = args.items.iter().all(|v| v.status.is_completed());
        let items = if all_done { Vec::new() } else { args.items };
        Ok(UpdatePlanOutput { items })
    }
}
