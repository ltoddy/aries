/// see more: https://agentskills.io/specification
use std::io;
use std::path::{Path, PathBuf};

use futures::stream::{self, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fs::walk_dir;

const FILENAME: &str = "SKILL.md";

pub async fn load() -> anyhow::Result<Vec<Frontmatter>> {
    let dir = std::env::home_dir().unwrap_or(PathBuf::from("~"));
    let dir = dir.join(".agents/skills");
    let entries = walk_dir(dir, true, true).await?;

    stream::iter(entries)
        .map(|entry| entry.join(FILENAME))
        .then(Frontmatter::parse)
        .try_collect()
        .await
        .map_err(Into::into)
}

#[derive(Error, Debug)]
pub enum ParseFrontmatterError {
    #[error("Failed to read SKILL.md file: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to parse SKILL.md's frontmatter: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Frontmatter not readably")]
    FrontmatterExist,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Frontmatter {
    name: String,
    description: String,
    // license: Option<String>,
    // compatibility: Option<String>,
    // metadata: Option<HashMap<String, serde_yaml::Value>>,
    // #[serde(rename = "allowed-tools")]
    // allowed_tools: Option<Vec<String>>,
}

impl Frontmatter {
    const DELIMITER: &str = "---";

    pub async fn parse(file_path: impl AsRef<Path>) -> Result<Self, ParseFrontmatterError> {
        let content = tokio::fs::read_to_string(file_path).await?;
        let mut parts = content.splitn(3, Self::DELIMITER);
        parts.next();

        if let Some(frontmatter) = parts.next() {
            let frontmatter = serde_yaml::from_str::<Self>(frontmatter)?;
            return Ok(frontmatter);
        }
        Err(ParseFrontmatterError::FrontmatterExist)
    }

    pub fn render(&self, file_path: impl AsRef<Path>) -> String {
        let name = &self.name;
        let description = &self.description;
        let location = file_path.as_ref().display();
        format!(
            r#"<skill>
  <name>{name}</name>
  <description>{description}</description>
  <location>{location}</location>
</skill>"#
        )
    }
}
