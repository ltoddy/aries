use std::io;

#[derive(thiserror::Error, Debug)]
pub enum SkillError {
    #[error("failed to load skills: {0}")]
    Load(#[from] io::Error),
    #[error("skill \"{name}\" not found. Available skills: {available}")]
    NotFound { name: String, available: String },
    #[error("skill \"{name}\" is not listed in available_skills. Available skills: {available}")]
    NotAllowed { name: String, available: String },
}

impl SkillError {
    pub fn not_found(name: impl Into<String>, available: impl Into<String>) -> Self {
        let name = name.into();
        let available = available.into();

        Self::NotFound { name, available }
    }

    pub fn not_allowed(name: impl Into<String>, available: impl Into<String>) -> Self {
        let name = name.into();
        let available = available.into();

        Self::NotAllowed { name, available }
    }
}
