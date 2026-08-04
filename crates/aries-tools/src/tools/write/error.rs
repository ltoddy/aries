use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("Failed to write file: {0}")]
    Io(#[from] std::io::Error),

    #[error(
        "The file {0} already exists and is not empty. Use the Edit or MultiEdit tool to modify it instead."
    )]
    FileNotEmpty(std::path::PathBuf),
}

impl WriteError {
    pub fn file_not_empty(file_path: impl AsRef<Path>) -> Self {
        let file_path = file_path.as_ref();

        Self::FileNotEmpty(file_path.to_path_buf())
    }
}
