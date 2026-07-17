#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("Failed to write file: {0}")]
    Io(#[from] std::io::Error),
}
