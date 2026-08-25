use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BatchOutput {
    pub results: Vec<ToolOutput>,
}

impl BatchOutput {
    pub fn new(results: Vec<ToolOutput>) -> Self {
        Self { results }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(format!("{} results", output.results.len()))
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolOutput {
    success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl ToolOutput {
    pub fn success(result: Value) -> Self {
        Self { success: true, result: Some(result), error: None }
    }

    pub fn failed(err: impl std::error::Error) -> Self {
        Self { success: false, result: None, error: Some(err.to_string()) }
    }
}
