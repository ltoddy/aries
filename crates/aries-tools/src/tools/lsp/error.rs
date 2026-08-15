#[derive(thiserror::Error, Debug)]
pub enum LspError {
    #[error("lsp operation failed: {0}")]
    OperationFailed(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
}
