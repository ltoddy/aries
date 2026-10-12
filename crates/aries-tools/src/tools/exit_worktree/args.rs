use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExitGitWorktreeArgs {
    pub action: ExitGitWorktreeAction,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExitGitWorktreeAction {
    Keep,
    Remove,
}

impl ExitGitWorktreeArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;
        Ok((format!("{:?}", args.action), None))
    }
}
