use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AskUserQuestionOutput {
    pub answers: Vec<String>,
}

impl AskUserQuestionOutput {
    pub fn new() -> Self {
        Self { answers: Vec::new() }
    }
}

impl Default for AskUserQuestionOutput {
    fn default() -> Self {
        Self::new()
    }
}
