use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolArgsRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct SkillArgs {
    pub name: String,
}

impl SkillArgs {
    pub fn title(&self) -> String {
        format!("Load skill {}", self.name)
    }
}

impl ToolArgsRender for SkillArgs {
    fn render_args(raw: &str) -> Result<(String, Option<String>), RenderError> {
        let args: Self = serde_json::from_str(raw)?;
        let first = args.name;
        Ok((first, None))
    }
}
