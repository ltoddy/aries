#[derive(thiserror::Error, Debug)]
pub enum EditError {
    #[error("Failed to edit file: {0}")]
    EditError(String),
}
