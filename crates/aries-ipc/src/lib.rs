mod token_usages;

use std::future::Future;
use std::io;
use std::path::Path;

use aries_proto::usage_service_server::UsageServiceServer;
use thiserror::Error;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

pub use crate::token_usages::TokenUsages;

#[derive(Debug, Error)]
pub enum ServeError {
    #[error("failed to connect database: {0}")]
    Database(#[from] toasty::Error),
    #[error("failed to bind Unix socket: {0}")]
    Bind(#[from] io::Error),
    #[error("gRPC server failed: {0}")]
    Transport(#[from] tonic::transport::Error),
}

pub async fn serve(
    root_dir: impl AsRef<Path>,
    shutdown: impl Future<Output = ()>,
) -> Result<(), ServeError> {
    let root_dir = root_dir.as_ref();
    let db = aries_persistence::connect(root_dir).await?;
    let listener = UnixListener::bind(root_dir.join("aries.sock"))?;

    Server::builder()
        .add_service(UsageServiceServer::new(TokenUsages::new(db)))
        .serve_with_incoming_shutdown(UnixListenerStream::new(listener), shutdown)
        .await?;
    Ok(())
}
