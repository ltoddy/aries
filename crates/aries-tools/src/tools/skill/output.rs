use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct SkillOutput {
    pub title: String,
    pub output: String,
    pub metadata: SkillMetadata,
}

impl SkillOutput {
    pub fn render_output(raw: &str) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_str(raw)?;
        Ok(output.output)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SkillMetadata {
    pub name: String,
    pub dir: PathBuf,
}
