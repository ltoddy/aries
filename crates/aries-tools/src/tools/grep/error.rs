#[derive(thiserror::Error, Debug)]
pub enum GrepError {
    #[error("invalid regex pattern {pattern:?}: {source}")]
    Regex { pattern: String, source: grep_regex::Error },

    #[error("invalid include glob pattern {pattern:?}: {source}")]
    IncludeGlob { pattern: String, source: globset::Error },

    #[error("failed to build include globset for pattern {pattern:?}: {source}")]
    IncludeGlobSet { pattern: String, source: globset::Error },

    #[error("search error: {0}")]
    Search(#[from] std::io::Error),

    #[error("internal error: result collector still has outstanding references")]
    CollectorStillShared,
}

impl GrepError {
    pub fn regex(pattern: impl Into<String>, source: grep_regex::Error) -> Self {
        let pattern = pattern.into();

        Self::Regex { pattern, source }
    }

    pub fn include_glob(pattern: impl Into<String>, source: globset::Error) -> Self {
        let pattern = pattern.into();

        Self::IncludeGlob { pattern, source }
    }

    pub fn include_globset(pattern: impl Into<String>, source: globset::Error) -> Self {
        let pattern = pattern.into();

        Self::IncludeGlobSet { pattern, source }
    }
}
