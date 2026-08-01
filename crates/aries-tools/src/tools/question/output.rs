use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AskUserQuestionOutput {
    pub answers: Vec<String>,
}

impl AskUserQuestionOutput {
    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(output.answers.join("\n"))
    }
}
