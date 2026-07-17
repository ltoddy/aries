#[derive(thiserror::Error, Debug)]
pub enum GrepError {
    #[error("Regex error: {0}")]
    Regex(#[from] regex_lite::Error),
    #[error("Globset error: {0}")]
    Globset(#[from] globset::Error),
}
