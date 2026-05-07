use std::fmt::{self, Display};
use std::io;
use std::path::PathBuf;

use anyhow::Result;
use itertools::Itertools;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

use crate::ext::skill::SkillInfo;
use crate::fs::{path_to_uri, walk_dir};

#[derive(Debug, Deserialize, Serialize)]
pub struct SkillArgs {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SkillOutput {
    pub title: String,
    pub output: String,
    pub metadata: SkillMetadata,
}

impl Display for SkillOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.output)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct SkillMetadata {
    pub name: String,
    pub dir: PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum SkillError {
    #[error("Failed to load skills: {0}")]
    Load(#[from] io::Error),
    #[error("Skill \"{name}\" not found. Available skills: {available}")]
    NotFound { name: String, available: String },
    #[error("Skill \"{name}\" is not listed in available_skills. Available skills: {available}")]
    NotAllowed { name: String, available: String },
}

impl SkillError {
    pub fn not_found(name: String, available: String) -> Self {
        Self::NotFound { name, available }
    }

    pub fn not_allowed(name: String, available: String) -> Self {
        Self::NotAllowed { name, available }
    }
}

pub const NAME: &str = "Skill";

pub struct SkillTool {
    skills: Vec<SkillInfo>,
}

impl SkillTool {
    pub fn new(skills: Vec<SkillInfo>) -> Self {
        Self { skills }
    }

    fn available_skills_display(&self) -> String {
        if self.skills.is_empty() {
            "none".to_string()
        } else {
            self.skills.iter().map(|s| &s.frontmatter.name).join(", ")
        }
    }
}

impl Tool for SkillTool {
    const NAME: &'static str = NAME;
    type Error = SkillError;
    type Args = SkillArgs;
    type Output = SkillOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: include_str!("descriptions/skill.txt").to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The name of the skill from available_skills"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let name = args.name;
        let available = self.available_skills_display();

        let skill = self
            .skills
            .iter()
            .find(|s| s.frontmatter.name == name)
            .ok_or_else(|| SkillError::not_allowed(name.clone(), available.clone()))?;

        let dir = skill
            .location
            .parent()
            .ok_or_else(|| SkillError::not_found(name.clone(), available.clone()))?;

        let entries = walk_dir(dir, true, true)?;
        let files = entries
            .iter()
            .filter(|e| e.is_file())
            .map(|f| format!("<file>{}</file>", f.display()))
            .join("\n");

        let output = [
            format!(r#"<skill_content name="{}">"#, skill.frontmatter.name),
            format!("# Skill: {}", skill.frontmatter.name),
            skill.body.clone(),
            format!("Base directory for this skill: {}", path_to_uri(dir)),
            "Relative paths in this skill (e.g., scripts/, reference/) are relative to this base directory.".to_owned(),
            "Note: file list is sampled.".to_owned(),
            "<skill_files>".to_owned(),
            files,
            "</skill_files>".to_owned(),
            "</skill_content>".to_owned(),
        ]
        .join("\n");

        Ok(SkillOutput {
            title: format!("Loaded skill: {name}"),
            output,
            metadata: SkillMetadata { name, dir: dir.to_path_buf() },
        })
    }
}
