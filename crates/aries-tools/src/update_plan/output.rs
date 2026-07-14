use serde::{Deserialize, Serialize};

use crate::update_plan::PlanEntry;

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdatePlanOutput {
    pub items: Vec<PlanEntry>,
}

impl UpdatePlanOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;
        if output.items.is_empty() {
            return Ok("Plan cleared.".to_owned());
        }

        let count = output.items.len();
        let lines = output.items.into_iter().map(|entry| format!("{entry}")).collect::<Vec<_>>();
        Ok(format!("Plan ({count} items):\n{}", lines.join("\n")))
    }
}
