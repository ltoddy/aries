use std::fmt;
use std::path::PathBuf;

use crate::context::GuardWriteError;

#[derive(Debug, Clone, Copy)]
pub enum MultiEditIoOperation {
    ReadFile,
    CreateParentDirectory,
    WriteFile,
}

impl fmt::Display for MultiEditIoOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let operation = match self {
            Self::ReadFile => "read file",
            Self::CreateParentDirectory => "create parent directory for",
            Self::WriteFile => "write file",
        };

        f.write_str(operation)
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MultiEditError {
    #[error("failed to {operation} {path}: {source}")]
    Io { operation: MultiEditIoOperation, path: PathBuf, source: std::io::Error },

    #[error("old_text and new_text cannot be identical")]
    IdenticalText,

    #[error("old_text cannot be empty unless the target file is empty or does not exist")]
    EmptyOldText,

    #[error("old_text not found in file (must match exactly including whitespace): {0:?}")]
    OldTextNotFound(String),

    #[error(transparent)]
    Guard(#[from] GuardWriteError),
}

impl MultiEditError {
    pub fn io(
        operation: MultiEditIoOperation,
        path: impl Into<PathBuf>,
        source: std::io::Error,
    ) -> Self {
        let path = path.into();

        Self::Io { operation, path, source }
    }

    pub fn identical_text() -> Self {
        MultiEditError::IdenticalText
    }

    pub fn empty_old_text() -> Self {
        MultiEditError::EmptyOldText
    }

    pub fn old_text_not_found(text: impl Into<String>) -> Self {
        Self::OldTextNotFound(text.into())
    }

    pub fn guard(err: GuardWriteError) -> Self {
        Self::Guard(err)
    }
}
