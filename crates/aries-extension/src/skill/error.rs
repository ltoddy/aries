use std::io;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseSkillError {
    #[error("Failed to read SKILL.md file: {0}")]
    Io(#[from] io::Error),
    #[error("Failed to parse SKILL.md's frontmatter: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Wrong SKILL.md format")]
    WrongFormat,
}
