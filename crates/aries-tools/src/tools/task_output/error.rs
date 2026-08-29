#[derive(thiserror::Error, Debug)]
pub enum TaskOutputError {
    #[error("task not found: {0}")]
    NotFound(String),
}

impl TaskOutputError {
    pub fn not_found(task_id: impl Into<String>) -> Self {
        Self::NotFound(task_id.into())
    }
}
