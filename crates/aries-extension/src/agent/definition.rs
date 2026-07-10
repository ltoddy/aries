use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Frontmatter {
    pub name: String,
    pub description: String,
    pub tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomAgentDefinition {
    location: PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl CustomAgentDefinition {
    pub fn new(
        location: impl AsRef<Path>,
        frontmatter: Frontmatter,
        body: impl Into<String>,
    ) -> Self {
        let location = location.as_ref().to_path_buf();
        let body = body.into();
        Self { location, frontmatter, body }
    }

    pub fn location(&self) -> &Path {
        &self.location
    }
}
