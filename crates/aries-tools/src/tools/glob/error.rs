#[derive(thiserror::Error, Debug)]
pub enum GlobError {
    #[error("globset error: {0}")]
    GlobsetError(#[from] globset::Error),
    #[error("walk error: {0}")]
    Walk(String),
    #[error("internal error: glob collector still has outstanding references")]
    CollectorStillShared,
}

impl GlobError {
    pub fn walk(s: impl Into<String>) -> GlobError {
        Self::Walk(s.into())
    }
}
