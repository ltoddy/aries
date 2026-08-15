#[derive(thiserror::Error, Debug)]
pub enum GlobError {
    #[error("globset error: {0}")]
    GlobsetError(#[from] globset::Error),
    #[error("walk error: {0}")]
    Walk(String),
}

impl GlobError {
    pub fn walk(s: impl Into<String>) -> GlobError {
        Self::Walk(s.into())
    }
}
