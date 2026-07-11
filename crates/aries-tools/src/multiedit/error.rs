#[derive(thiserror::Error, Debug)]
pub enum MultiEditError {
    #[error("Failed to edit file: {0}")]
    EditError(String),
}
