use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MonitorOutput {
    pub task_id: String,
    pub message: String,
}

impl MonitorOutput {
    pub fn new(task_id: impl Into<String>, message: impl Into<String>) -> Self {
        let task_id = task_id.into();
        let message = message.into();

        Self { task_id, message }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!("{}\ntask_id: {}", output.message, output.task_id))
    }
}
