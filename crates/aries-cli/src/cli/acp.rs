use agent_client_protocol::{AgentSideConnection, Client};
use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use tokio::sync::mpsc;
use tokio::{io, task};
use tokio_util::compat::{TokioAsyncReadCompatExt as _, TokioAsyncWriteCompatExt as _};
use tracing::error;

pub async fn execute(gctx: GlobalContext) -> anyhow::Result<()> {
    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let config = loader.load_or_setup().await?;

    let outgoing = io::stdout().compat_write();
    let incoming = io::stdin().compat();

    let local_set = task::LocalSet::new();
    local_set
        .run_until(async move {
            let (sender, mut receiver) = mpsc::unbounded_channel();
            let agent = crate::acp::AcpImpl::new(gctx, config, sender).await;

            let (conn, handle_io) = AgentSideConnection::new(agent, outgoing, incoming, |fut| {
                task::spawn_local(fut);
            });

            task::spawn_local(async move {
                while let Some((session_notification, tx)) = receiver.recv().await {
                    match conn.session_notification(session_notification).await {
                        Ok(_) => {
                            let _ = tx.send(());
                        },
                        Err(err) => {
                            error!("Failed to send session notification: {err}");
                            break;
                        },
                    }
                }
            });

            handle_io.await
        })
        .await?;

    Ok(())
}
