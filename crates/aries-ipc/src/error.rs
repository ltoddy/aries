use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServeError {
    #[error("failed to initialize database: {0}")]
    Database(#[from] toasty::Error),
    #[error("failed to prepare Unix socket: {0}")]
    Bind(#[from] io::Error),
    #[error("gRPC server failed: {0}")]
    Transport(#[from] tonic::transport::Error),
}

impl ServeError {
    pub fn database(err: toasty::Error) -> Self {
        Self::Database(err)
    }

    pub fn bind(err: io::Error) -> Self {
        Self::Bind(err)
    }

    pub fn transport(err: tonic::transport::Error) -> Self {
        Self::Transport(err)
    }
}
