use std::io;

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
