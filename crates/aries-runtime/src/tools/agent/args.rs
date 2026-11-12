use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentArgs {
    pub name: String,
    pub description: String,
    pub prompt: String,
}

impl AgentArgs {
    pub fn title(&self) -> String {
        format!("Launch {} subagent: {}", self.name, self.description)
    }
}

impl AgentArgs {
    pub fn render_args(
        raw: impl AsRef<str>,
    ) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw.as_ref())?;

        let mut first = args.description;
        first.push_str(&format!(", name = {}", args.name));
        Ok((first, Some(args.prompt)))
    }
}
