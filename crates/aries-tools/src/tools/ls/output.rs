use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct LsOutput {
    pub entries: Vec<String>,
}

impl LsOutput {
    pub fn new(entries: Vec<String>) -> Self {
        Self { entries }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(output.entries.join("\n"))
    }
}
