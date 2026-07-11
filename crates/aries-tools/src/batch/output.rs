use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{RenderError, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct BatchOutput {
    pub results: Vec<Value>,
}

impl ToolOutputRender for BatchOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(format!("{} results", output.results.len()))
    }
}
