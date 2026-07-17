#[derive(thiserror::Error, Debug)]
pub enum LsError {
    #[error("Failed to read directory: {0}")]
    Io(#[from] std::io::Error),
}
