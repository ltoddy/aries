use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ext::skill::ParseSkillError;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillDefinition {
    pub location: PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl SkillDefinition {
    pub async fn parse(file_path: impl AsRef<Path>) -> Result<Self, ParseSkillError> {
        let location = file_path.as_ref().to_path_buf();

        let content = tokio::fs::read_to_string(&location).await?;
        let mut parts = content.splitn(3, Frontmatter::DELIMITER);
        parts.next();

        match (parts.next(), parts.next()) {
            (Some(frontmatter), Some(body)) => {
                let frontmatter = serde_yaml::from_str::<Frontmatter>(frontmatter)?;
                let body = body.to_owned();
                Ok(Self { location, frontmatter, body })
            },
            _ => Err(ParseSkillError::WrongFormat),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Frontmatter {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: Option<HashMap<String, serde_yaml::Value>>,
    #[serde(rename = "allowed-tools")]
    pub allowed_tools: Option<Vec<String>>,
}

impl Frontmatter {
    const DELIMITER: &str = "---";

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
