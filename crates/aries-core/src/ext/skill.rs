/// see more: https://agentskills.io/specification
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use futures::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fs::walk_dirs;

pub struct SkillFilesLoader {
    roots: Vec<PathBuf>,
}

impl SkillFilesLoader {
    pub const FILENAME: &str = "SKILL.md";

    pub fn new(cwd: impl AsRef<Path>) -> Self {
        let cwd = cwd.as_ref();
        let home_dir = std::env::home_dir().unwrap_or_else(|| PathBuf::from("~"));
        let roots = vec![home_dir.join(".agents").join("skills"), cwd.to_path_buf()];

        Self { roots }
    }

    pub async fn load(self) -> anyhow::Result<Vec<SkillInfo>> {
        let entries = walk_dirs(&self.roots, true, true)?;

        let skills = stream::iter(entries.into_iter().filter(|entry| entry.is_dir()))
            .filter_map(
                |entry| async move { SkillInfo::parse(entry.join(Self::FILENAME)).await.ok() },
            )
            .collect()
            .await;

        Ok(skills)
    }
}

#[derive(Error, Debug)]
pub enum ParseSkillError {
    #[error("Failed to read SKILL.md file: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to parse SKILL.md's frontmatter: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Wrong SKILL.md format")]
    WrongFormat,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SkillInfo {
    pub location: PathBuf,
    pub frontmatter: Frontmatter,
    pub body: String,
}

impl SkillInfo {
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
