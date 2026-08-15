#[derive(thiserror::Error, Debug)]
pub enum CodeSearchError {
    #[error("failed to perform code search: {0}")]
    SearchError(String),
}
