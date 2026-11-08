use std::fmt;
use std::path::PathBuf;

use crate::context::GuardWriteError;

#[derive(Debug, Clone, Copy)]
pub enum WriteOperation {
    CreateParentDirectory,
    ReadMetadata,
    WriteFile,
}

impl fmt::Display for WriteOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let operation = match self {
            Self::CreateParentDirectory => "create parent directory for",
            Self::ReadMetadata => "read metadata for",
            Self::WriteFile => "write file",
        };

        f.write_str(operation)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("failed to {operation} {path}: {source}")]
    Io { operation: WriteOperation, path: PathBuf, source: std::io::Error },

    #[error(
        "the file {0} already exists and is not empty. use the edit or multiedit tool to modify it instead."
    )]
    FileNotEmpty(PathBuf),

    #[error(transparent)]
    GuardWrite(#[from] GuardWriteError),
}

impl WriteError {
    pub fn io(operation: WriteOperation, path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        let path = path.into();

        Self::Io { operation, path, source }
    }

    pub fn file_not_empty(file_path: impl Into<PathBuf>) -> Self {
        let file_path = file_path.into();

        Self::FileNotEmpty(file_path)
    }
}
