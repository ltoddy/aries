use crate::context::GuardWriteError;

#[derive(thiserror::Error, Debug)]
pub enum MultiEditError {
    #[error("failed to edit file: {0}")]
    Io(#[from] std::io::Error),

    #[error("old_text and new_text cannot be identical")]
    IdenticalText,

    #[error("old_text not found in file (must match exactly including whitespace): {0:?}")]
    OldTextNotFound(String),

    #[error(transparent)]
    Guard(#[from] GuardWriteError),
}

impl MultiEditError {
    pub fn identical_text() -> Self {
        MultiEditError::IdenticalText
    }

    pub fn old_text_not_found(text: impl Into<String>) -> Self {
        Self::OldTextNotFound(text.into())
    }

    pub fn guard(err: GuardWriteError) -> Self {
        Self::Guard(err)
    }
}
