use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize)]
pub struct BatchOutput {
    pub results: Vec<Value>,
}

impl BatchOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(format!("{} results", output.results.len()))
    }
}
