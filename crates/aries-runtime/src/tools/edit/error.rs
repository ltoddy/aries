use std::fmt;
use std::path::PathBuf;

use crate::context::GuardWriteError;

#[derive(Debug, Clone, Copy)]
pub enum EditIoOperation {
    ReadFile,
    WriteFile,
}

impl fmt::Display for EditIoOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let operation = match self {
            Self::ReadFile => "read file",
            Self::WriteFile => "write file",
        };

        f.write_str(operation)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum EditError {
    #[error("failed to {operation} {path}: {source}")]
    Io { operation: EditIoOperation, path: PathBuf, source: std::io::Error },

    #[error("file does not exist: {0}. use the write tool to create new files")]
    FileNotFound(PathBuf),

    #[error("old_text and new_text cannot be identical")]
    IdenticalText,

    #[error("old_text not found in content")]
    OldTextNotFound,

    #[error(
        "found {count} matches for old_text, but replace_all is false. provide more surrounding lines in old_text to identify a unique match, or set replace_all to true"
    )]
    MultipleMatches { count: usize },

    #[error(transparent)]
    Guard(#[from] GuardWriteError),
}

impl EditError {
    pub fn io(
        operation: EditIoOperation,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        let path = path.into();

        Self::Io { operation, path, source }
    }

    pub fn file_not_found(path: impl Into<PathBuf>) -> Self {
        Self::FileNotFound(path.into())
    }

    pub fn old_text_not_found() -> Self {
        Self::OldTextNotFound
    }

    pub fn identical_text() -> Self {
        Self::IdenticalText
    }

    pub fn multiple_matches(count: usize) -> Self {
        Self::MultipleMatches { count }
    }
}
