#[derive(thiserror::Error, Debug)]
pub enum GrepError {
    #[error("regex error: {0}")]
    Regex(#[from] regex_lite::Error),
    #[error("globset error: {0}")]
    Globset(#[from] globset::Error),
}
