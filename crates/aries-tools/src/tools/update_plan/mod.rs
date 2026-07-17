mod args;
mod entry;
mod error;
mod output;

use rig_core::tool::Tool;
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
                            "content": { "type": "string" },
                            "priority": { "type": "string", "enum": ["high", "medium", "low"] },
                            "status": { "type": "string", "enum": ["pending", "in_progress", "completed"] }
                        },
                        "required": ["content", "priority", "status"]
                    }
                }
            },
            "required": ["items"]
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        Ok(UpdatePlanOutput { items: args.items })
    }
}
