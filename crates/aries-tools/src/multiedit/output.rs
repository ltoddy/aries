use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct MultiEditOutput {
    pub success: bool,
    pub message: String,
}

impl ToolOutputRender for MultiEditOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output = serde_json::from_str::<MultiEditOutput>(raw)?;
        Ok(output.message)
    }
}
