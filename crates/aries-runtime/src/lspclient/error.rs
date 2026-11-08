use std::io;

use tokio::sync::oneshot;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("LSP request was canceled")]
    RequestCanceled(#[from] oneshot::error::RecvError),
    #[error("LSP request timed out")]
    RequestTimedOut(#[from] tokio::time::error::Elapsed),
    #[error("failed to open stdin for LSP process")]
    MissingStdin,
    #[error("failed to open stdout for LSP process")]
    MissingStdout,
}
