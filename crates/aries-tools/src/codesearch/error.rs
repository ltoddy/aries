#[derive(thiserror::Error, Debug)]
pub enum CodeSearchError {
    #[error("Failed to perform code search: {0}")]
    SearchError(String),
}
