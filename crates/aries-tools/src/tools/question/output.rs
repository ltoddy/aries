use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AskUserQuestionOutput {
    pub answers: Vec<String>,
}

impl AskUserQuestionOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.answers.join("\n"))
    }
}
