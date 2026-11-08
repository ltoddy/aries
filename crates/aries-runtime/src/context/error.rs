#[derive(thiserror::Error, Debug)]
pub enum GuardWriteError {
    #[error("file has not been read yet. read it first before writing to it")]
    NotRead,

    #[error(
        "file has been modified since read, either by the user or by a linter. read it again before attempting to write it"
    )]
    ModifiedSinceRead,
}
