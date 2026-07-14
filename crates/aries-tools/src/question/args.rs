use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct AskUserQuestionOption {
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AskUserQuestionArgs {
    pub question: String,
    pub options: Option<Vec<AskUserQuestionOption>>,
    #[serde(default)]
    pub multiple: bool,
    #[serde(default = "default_custom")]
    pub custom: bool,
}

impl AskUserQuestionArgs {
    pub fn title(&self) -> String {
        format!("Ask user: {}", self.question)
    }
}

impl AskUserQuestionArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.question;
        Ok((first, None))
    }
}

fn default_custom() -> bool {
    true
}
