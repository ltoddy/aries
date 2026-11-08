use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SkillArgs {
    pub name: String,
}

impl SkillArgs {
    pub fn title(&self) -> String {
        format!("Load skill {}", self.name)
    }
}

impl SkillArgs {
    pub fn render_args(raw: &str) -> Result<(String, Option<String>), serde_json::Error> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.name;
        Ok((first, None))
    }
}
