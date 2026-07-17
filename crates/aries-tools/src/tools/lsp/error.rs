#[derive(thiserror::Error, Debug)]
pub enum LspError {
    #[error("LSP operation failed: {0}")]
    OperationFailed(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}
