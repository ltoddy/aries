#[derive(thiserror::Error, Debug)]
pub enum ReadError {
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),
}
