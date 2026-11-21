mod error;
mod service;
mod socket;

use std::future::Future;
use std::path::Path;

use aries_proto::aries_service_server::AriesServiceServer;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;

pub use crate::error::ServeError;
pub use crate::service::AriesServiceImpl;

pub async fn serve(
    root_dir: impl AsRef<Path>,
    shutdown: impl Future<Output = ()>,
) -> Result<(), ServeError> {
    let root_dir = root_dir.as_ref();
    let (_socket, listener) = socket::Socket::bind(root_dir).await.map_err(ServeError::bind)?;
    let db = aries_persistence::connect(root_dir).await.map_err(ServeError::database)?;
    tracing::info!(path = %root_dir.join("aries.sock").display(), "Aries IPC server listening");

    Server::builder()
        .add_service(AriesServiceServer::new(AriesServiceImpl::new(db)))
        .serve_with_incoming_shutdown(UnixListenerStream::new(listener), shutdown)
        .await
        .map_err(ServeError::transport)?;
    tracing::info!("Aries IPC server stopped");
    Ok(())
}
