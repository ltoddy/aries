use serde::{Deserialize, Serialize};

use crate::update_plan::PlanEntry;

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePlanOutput {
    pub items: Vec<PlanEntry>,
}

impl UpdatePlanOutput {
    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        if output.items.is_empty() {
            return Ok(String::from(
                "Plan cleared. Continue to use the plan to track your progress if applicable.",
            ));
        }

        let count = output.items.len();
        let lines = output.items.into_iter().map(|entry| format!("{entry}")).collect::<Vec<_>>();
        Ok(format!(
            "Plan updated successfully. Ensure that you continue to use the plan to track your \
             progress.\n\nPlan ({count} items):\n{}",
            lines.join("\n")
        ))
    }
}
