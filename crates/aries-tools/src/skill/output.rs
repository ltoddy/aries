use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{RenderError, ToolOutputRender};

#[derive(Debug, Deserialize, Serialize)]
pub struct SkillOutput {
    pub title: String,
    pub output: String,
    pub metadata: SkillMetadata,
}

impl ToolOutputRender for SkillOutput {
    fn render_output(raw: &str) -> Result<String, RenderError> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.output)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SkillMetadata {
    pub name: String,
    pub dir: PathBuf,
}
