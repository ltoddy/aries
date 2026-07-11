#[derive(thiserror::Error, Debug)]
pub enum BashError {
    #[error("Failed to execute command: {0}")]
    ExecutionFailed(String),
}
