use serde::{Deserialize, Serialize};

const NOTICE: &str = "<system-reminder>File truncated; more lines follow. Use offset/limit to continue reading.</system-reminder>";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReadOutput {
    pub content: String,
    #[serde(default)]
    pub truncated: bool,
}

impl ReadOutput {
    pub fn new(content: impl Into<String>, truncated: bool) -> Self {
        let content = content.into();
        Self { content, truncated }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        if output.truncated {
            return Ok(format!("{}\n{NOTICE}", output.content));
        }

        Ok(output.content)
    }
}
