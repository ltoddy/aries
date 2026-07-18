use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum ReadError {
    #[error("Failed to read file: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "Path is a directory, not a file: {0}. To list a directory, use the Ls or Glob tool instead."
    )]
    IsADirectory(PathBuf),
}

impl ReadError {
    pub fn is_a_directory(path: PathBuf) -> ReadError {
        ReadError::IsADirectory(path)
    }
}
