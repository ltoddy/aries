use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub result: String,
}

impl ToolOutputRender for AgentOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.result)
    }
}
