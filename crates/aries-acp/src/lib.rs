pub mod agent;
mod logger;

use agent_client_protocol::{AgentSideConnection, Client};
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use tokio::sync::mpsc;
use tokio_util::compat::{TokioAsyncReadCompatExt as _, TokioAsyncWriteCompatExt as _};
use tracing::{error, info};

pub async fn run(gctx: GlobalContext, config: AriesConfig) -> anyhow::Result<()> {
    let _guard = logger::init(&gctx.config_dir);

    info!("Current directori is: {}", gctx.current_dir.display());
    let outgoing = tokio::io::stdout().compat_write();
    let incoming = tokio::io::stdin().compat();

    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            let (sender, mut receiver) = mpsc::unbounded_channel();
            let agent = agent::Agent::new(gctx, config, sender);

            let (conn, handle_io) = AgentSideConnection::new(agent, outgoing, incoming, |fut| {
                tokio::task::spawn_local(fut);
            });

            tokio::task::spawn_local(async move {
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
