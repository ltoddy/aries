use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SkillOutput {
    pub output: String,
    pub metadata: SkillMetadata,
}

impl SkillOutput {
    pub fn new(output: impl Into<String>, metadata: SkillMetadata) -> Self {
        let output = output.into();

        Self { output, metadata }
    }

    pub fn render_output(raw: serde_json::Value) -> Result<String, serde_json::Error> {
        let output: Self = serde_json::from_value(raw)?;
        Ok(output.output)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SkillMetadata {
    pub name: String,
    pub dir: PathBuf,
}

impl SkillMetadata {
    pub fn new(name: impl Into<String>, dir: impl AsRef<Path>) -> Self {
        let name = name.into();
        let dir = dir.as_ref();

        Self { name, dir: dir.to_owned() }
    }
}
