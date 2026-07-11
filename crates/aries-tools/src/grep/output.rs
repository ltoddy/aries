use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct GrepOutput {
    pub matches: Vec<String>,
}

impl ToolOutputRender for GrepOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.matches.join("\n"))
    }
}
