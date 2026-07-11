use aries_tools::{RenderError, ToolOutputRender};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentOutput {
    pub task_id: String,
    pub result: String,
}

impl ToolOutputRender for AgentOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.result)
    }
}
