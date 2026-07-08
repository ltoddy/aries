use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Frontmatter {
    pub name: String,
    pub description: String,
    pub tools: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CustomAgentDefinition {
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl CustomAgentDefinition {
    pub fn new(frontmatter: Frontmatter, body: impl Into<String>) -> Self {
        let body = body.into();
        Self { frontmatter, body }
    }
}
