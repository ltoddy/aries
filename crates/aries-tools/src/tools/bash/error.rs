#[derive(thiserror::Error, Debug)]
pub enum BashError {
    #[error("failed to execute command: {0}")]
    Io(#[from] std::io::Error),

    #[error("command timed out after {0}ms")]
    Timeout(u64),
}
