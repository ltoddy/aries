use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct MultiEditOutput {
    pub success: bool,
    pub message: String,
}

impl MultiEditOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output = serde_json::from_str::<MultiEditOutput>(raw)?;
        Ok(output.message)
    }
}
