use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BashArgs {
    pub command: String,
    #[serde(default)]
    pub timeout: Option<u64>,
    #[serde(default)]
    pub description: Option<String>,
}

impl BashArgs {
    pub fn title(&self) -> String {
        let command = self.command.trim();
        if command.is_empty() {
            "Run a shell command".to_owned()
        } else {
            format!("Run shell command: {command}")
        }
    }
}

impl BashArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;
        Ok((args.command, args.description))
    }
}
