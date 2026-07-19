use std::path::PathBuf;

use crate::context::GuardWriteError;

#[derive(thiserror::Error, Debug)]
pub enum EditError {
    #[error("Failed to edit file: {0}")]
    Io(#[from] std::io::Error),

    #[error("File does not exist: {0}. Use Write tool to create new files.")]
    FileNotFound(PathBuf),

    #[error("old_text and new_text cannot be identical")]
    IdenticalText,

    #[error("old_text not found in content")]
    OldTextNotFound,

    #[error(
        "Found {count} matches for old_text, but replace_all is false. Provide more surrounding lines in old_text to identify a unique match, or set replace_all to true."
    )]
    MultipleMatches { count: usize },

    #[error(transparent)]
    Guard(#[from] GuardWriteError),
}

impl EditError {
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
