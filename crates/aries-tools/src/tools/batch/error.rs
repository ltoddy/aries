#[derive(thiserror::Error, Debug)]
pub enum BatchError {
    #[error("batch execution failed: {0}")]
    ExecutionError(String),
}
