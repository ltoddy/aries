#[derive(thiserror::Error, Debug)]
pub enum GlobError {
    #[error("invalid glob pattern {pattern:?}: {source}")]
    Pattern { pattern: String, source: globset::Error },

    #[error("failed to build glob matcher for pattern {pattern:?}: {source}")]
    GlobSet { pattern: String, source: globset::Error },
}

impl GlobError {
    pub fn pattern(pattern: impl Into<String>, source: globset::Error) -> GlobError {
        let pattern = pattern.into();

        Self::Pattern { pattern, source }
    }

    pub fn glob_set(pattern: impl Into<String>, source: globset::Error) -> GlobError {
        let pattern = pattern.into();

        Self::GlobSet { pattern, source }
    }
}
