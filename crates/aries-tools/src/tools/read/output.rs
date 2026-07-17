use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct ReadOutput {
    pub content: String,
}

impl ReadOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.content)
    }
}
