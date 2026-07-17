#[derive(thiserror::Error, Debug)]
pub enum WebSearchError {
    #[error("Failed to perform web search: {0}")]
    SearchError(String),
}
