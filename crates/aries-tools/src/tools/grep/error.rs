use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum GrepError {
    #[error("invalid regex pattern {pattern:?}: {source}")]
    Regex { pattern: String, source: grep_regex::Error },

    #[error("invalid include glob pattern {pattern:?}: {source}")]
    IncludeGlob { pattern: String, source: globset::Error },

    #[error("failed to search {}: {source}", path.display())]
    Search { path: PathBuf, source: std::io::Error },

    #[error("failed to walk search directory: {0}")]
    Walk(#[from] ignore::Error),

    #[error("search task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

impl GrepError {
    pub fn regex(pattern: impl Into<String>, source: grep_regex::Error) -> Self {
        Self::Regex { pattern: pattern.into(), source }
    }

    pub fn include_glob(pattern: impl Into<String>, source: globset::Error) -> Self {
        Self::IncludeGlob { pattern: pattern.into(), source }
    }

    pub fn search(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Search { path: path.into(), source }
    }

    pub fn walk(source: ignore::Error) -> Self {
        Self::Walk(source)
    }

    pub fn task(source: tokio::task::JoinError) -> Self {
        Self::Task(source)
    }
}
