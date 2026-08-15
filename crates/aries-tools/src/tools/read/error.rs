use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum ReadError {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),

    #[error("path is a directory, not a file: {0}. to list a directory, use the glob tool instead")]
    IsADirectory(PathBuf),
}

impl ReadError {
    pub fn is_a_directory(path: PathBuf) -> ReadError {
        ReadError::IsADirectory(path)
    }
}
