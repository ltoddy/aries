use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebFetchOutput {
    pub content: String,
}

impl WebFetchOutput {
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        Self { content }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(output.content)
    }
}
