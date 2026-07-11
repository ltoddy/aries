#[derive(thiserror::Error, Debug)]
pub enum WebFetchError {
    #[error("Failed to fetch web content: {0}")]
    FetchError(String),
}
