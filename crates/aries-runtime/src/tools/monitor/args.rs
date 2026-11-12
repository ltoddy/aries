use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MonitorArgs {
    pub command: String,
    #[serde(default)]
    pub description: Option<String>,
}

impl MonitorArgs {
    pub fn title(&self) -> String {
        format!("Monitor command: {}", self.command.trim())
    }

    pub fn render_args(
        raw: impl AsRef<str>,
    ) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw.as_ref())?;
        Ok((args.command, args.description))
    }
}
