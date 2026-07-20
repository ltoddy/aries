#[derive(thiserror::Error, Debug)]
pub enum BashError {
    #[error("Failed to execute command: {0}")]
    Io(#[from] std::io::Error),

    #[error("Command timed out after {0}ms")]
    Timeout(u64),
}
