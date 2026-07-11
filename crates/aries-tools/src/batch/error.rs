#[derive(thiserror::Error, Debug)]
pub enum BatchError {
    #[error("Batch execution failed: {0}")]
    ExecutionError(String),
}
