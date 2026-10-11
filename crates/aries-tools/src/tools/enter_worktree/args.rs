use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EnterGitWorktreeArgs {
    pub name: Option<String>,
}

impl EnterGitWorktreeArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;
        Ok((args.name.unwrap_or_else(|| "auto".to_owned()), None))
    }
}
