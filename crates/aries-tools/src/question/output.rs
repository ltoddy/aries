use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct AskUserQuestionOutput {
    pub answers: Vec<String>,
}

impl ToolOutputRender for AskUserQuestionOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.answers.join("\n"))
    }
}
