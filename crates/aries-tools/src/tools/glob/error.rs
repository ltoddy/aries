#[derive(thiserror::Error, Debug)]
pub enum GlobError {
    #[error("Globset error: {0}")]
    GlobsetError(#[from] globset::Error),
    #[error("Walk error: {0}")]
    Walk(String),
}
