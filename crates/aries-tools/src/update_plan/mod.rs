mod args;
mod error;
mod output;

use rig_core::completion::ToolDefinition;
use rig_core::tool::Tool;

pub use self::args::{PlanEntry, PlanEntryPriority, PlanEntryStatus, UpdatePlanArgs};
pub use self::error::UpdatePlanError;
pub use self::output::UpdatePlanOutput;

pub const NAME: &str = "UpdatePlan";

pub struct UpdatePlanTool {
    on_update: Box<dyn Fn(Vec<PlanEntry>) -> Result<(), UpdatePlanError> + Send + Sync>,
}

impl UpdatePlanTool {
    pub fn new(
        on_update: impl Fn(Vec<PlanEntry>) -> Result<(), UpdatePlanError> + Send + Sync + 'static,
    ) -> Self {
        Self { on_update: Box::new(on_update) }
    }
}

impl Tool for UpdatePlanTool {
    const NAME: &'static str = NAME;
    type Error = UpdatePlanError;
    type Args = UpdatePlanArgs;
    type Output = UpdatePlanOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_owned(),
            description: include_str!("description.md").to_owned(),
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
        (self.on_update)(args.items)?;
        Ok(UpdatePlanOutput { ok: true })
    }
}
