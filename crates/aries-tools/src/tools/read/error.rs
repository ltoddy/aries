use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub enum ReadOperation {
    OpenFile,
    ReadMetadata,
    InspectFileType,
    SeekToStart,
    ReadLine,
    CheckTruncated,
}

impl fmt::Display for ReadOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let operation = match self {
            Self::OpenFile => "open file",
            Self::ReadMetadata => "read metadata for",
            Self::InspectFileType => "inspect file type for",
            Self::SeekToStart => "seek to start of",
            Self::ReadLine => "read line from",
            Self::CheckTruncated => "check whether file is truncated for",
        };

        f.write_str(operation)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ReadError {
    #[error("failed to {operation} {path}: {source}")]
    Io { operation: ReadOperation, path: PathBuf, source: std::io::Error },

    #[error("limit must be greater than 0")]
    InvalidLimit,

    #[error("path is a directory, not a file: {0}. to list a directory, use the glob tool instead")]
    IsADirectory(PathBuf),

    #[error("cannot read binary file: {0}")]
    BinaryFile(PathBuf),
}

impl ReadError {
    pub fn io(
        operation: ReadOperation,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> ReadError {
        let path = path.into();

        ReadError::Io { operation, path, source }
    }

    pub fn invalid_limit() -> ReadError {
        ReadError::InvalidLimit
    }

    pub fn is_a_directory(path: PathBuf) -> ReadError {
        ReadError::IsADirectory(path)
    }

    pub fn binary_file(path: PathBuf) -> ReadError {
        ReadError::BinaryFile(path)
    }
}
