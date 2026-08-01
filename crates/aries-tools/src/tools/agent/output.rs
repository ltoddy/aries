use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AgentOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub result: String,
}

impl AgentOutput {
    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(output.result)
    }
}
