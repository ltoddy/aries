use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::tool::ToolList;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillDefinition {
    pub location: PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl SkillDefinition {
    pub fn new(
        location: impl AsRef<Path>,
        frontmatter: Frontmatter,
        body: impl Into<String>,
    ) -> Self {
        let location = location.as_ref();
        let body = body.into();

        Self { location: location.to_owned(), frontmatter, body }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Frontmatter {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: Option<HashMap<String, serde_yaml::Value>>,
    #[serde(rename = "allowed-tools", default)]
    pub allowed_tools: ToolList,
}

impl Frontmatter {
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            license: None,
            compatibility: None,
            metadata: None,
            allowed_tools: ToolList::default(),
        }
    }

    pub fn render(&self, file_path: impl AsRef<Path>) -> String {
        let name = &self.name;
        let description = &self.description;
        let location = file_path.as_ref().display();

        [
            "<skill>",
            format!("  <name>{name}</name>").as_str(),
            format!("  <description>{description}</description>").as_str(),
            format!("  <location>{location}</location>").as_str(),
            "</skill>",
        ]
        .join("\n")
    }
}
