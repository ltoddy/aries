/// see more: https://agentskills.io/specification
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use futures::stream::{self, StreamExt, TryStreamExt};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::fs::walk_dir;

pub const FILENAME: &str = "SKILL.md";

pub async fn load() -> anyhow::Result<Vec<SkillInfo>> {
    let dir = std::env::home_dir().unwrap_or(PathBuf::from("~"));
    let dir = dir.join(".agents/skills");
    let entries = walk_dir(dir, false, true).await?;

    stream::iter(entries.into_iter().filter(|entry| entry.is_dir()))
        .map(|entry| entry.join(FILENAME))
        .then(SkillInfo::parse)
        .try_collect()
        .await
        .map_err(Into::into)
}

pub fn render_available_skills(skills: &[SkillInfo]) -> String {
    if skills.is_empty() {
        return "<available_skills>\n(none)\n</available_skills>".to_string();
    }

    let skills = skills.iter().map(|s| s.frontmatter.render(&s.location)).join("\n");
    format!("<available_skills>\n{skills}\n</available_skills>",)
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
        format!(
            r#"<skill>
  <name>{name}</name>
  <description>{description}</description>
  <location>{location}</location>
</skill>"#
        )
    }
}
