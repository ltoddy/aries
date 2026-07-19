#[derive(thiserror::Error, Debug)]
pub enum GuardWriteError {
    #[error("File has not been read yet. Read it first before writing to it.")]
    NotRead,

    #[error(
        "File has been modified since read, either by the user or by a linter. Read it again before attempting to write it."
    )]
    ModifiedSinceRead,
}
