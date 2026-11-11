use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentOutput {
    pub task_id: String,
    pub result: String,
    pub transcript_path: PathBuf,
}

impl AgentOutput {
    pub fn new(
        task_id: impl Into<String>,
        result: impl Into<String>,
        transcript_path: impl Into<PathBuf>,
    ) -> Self {
        let task_id = task_id.into();
        let result = result.into();
        let transcript_path = transcript_path.into();

        Self { task_id, result, transcript_path }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(output.result)
    }
}
