mod error;

use std::time::Duration;

use aries_init::GlobalContext;
use tokio::signal::unix::{self, SignalKind};
use tokio::sync::oneshot;
use tokio::time;

use crate::error::Error;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt().with_ansi(false).with_writer(std::io::stderr).init();

    let gctx = GlobalContext::new().await;
    let mut terminate =
        unix::signal(SignalKind::terminate()).map_err(|source| Error::signal("SIGTERM", source))?;
    let mut interrupt =
        unix::signal(SignalKind::interrupt()).map_err(|source| Error::signal("SIGINT", source))?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = aries_ipc::serve(gctx.root_dir(), async move {
        let _ = shutdown_rx.await;
    });
    tokio::pin!(server);
    let result = tokio::select! {
        result = &mut server => result,
        _ = async {
            tokio::select! {
                _ = terminate.recv() => {},
                _ = interrupt.recv() => {},
            }
        } => {
            tracing::info!("shutdown requested");
            let _ = shutdown_tx.send(());
            match time::timeout(SHUTDOWN_TIMEOUT, &mut server).await {
                Ok(result) => result,
                Err(_) => {
                    tracing::warn!("shutdown deadline exceeded; cancelling IPC server");
                    return Ok(());
                },
            }
        },
    };
    if let Err(source) = result {
        tracing::error!(error = %source, "Aries daemon failed");
        return Err(Error::serve(source));
    }
    Ok(())
}
