use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BatchOutput {
    pub results: Vec<Value>,
}

impl BatchOutput {
    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!("{} results", output.results.len()))
    }
}
