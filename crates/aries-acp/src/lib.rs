pub mod agent;

use agent_client_protocol::{AgentSideConnection, Client};
use aries_context::GlobalContext;
use aries_core::orchestrate::OrchestrateAgent;
use tokio::sync::mpsc;
use tokio_util::compat::{TokioAsyncReadCompatExt as _, TokioAsyncWriteCompatExt as _};
use tracing::{error, info};

pub async fn run(gctx: GlobalContext, orchestrate: OrchestrateAgent) -> anyhow::Result<()> {
    info!("Current directori is: {}", gctx.current_dir.display());
    let outgoing = tokio::io::stdout().compat_write();
    let incoming = tokio::io::stdin().compat();

    let local_set = tokio::task::LocalSet::new();
    local_set
        .run_until(async move {
            let (sender, mut receiver) = mpsc::unbounded_channel();
            let agent = agent::Agent::new(orchestrate, sender);

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

// struct AriesAgent {
//     sender:
// mpsc::UnboundedSender<(agent_client_protocol::SessionNotification,
// oneshot::Sender<()>)>,     next_session_id: Cell<u64>,
// }
//
// impl AriesAgent {
//     fn new(sender:
// mpsc::UnboundedSender<(agent_client_protocol::SessionNotification,
// oneshot::Sender<()>)>) -> Self {         Self { sender, next_session_id:
// Cell::new(0) }     }
// }
//
// #[async_trait(?Send)]
// impl agent_client_protocol::Agent for AriesAgent {
//     async fn initialize(
//         &self,
//         arguments: agent_client_protocol::InitializeRequest,
//     ) -> Result<agent_client_protocol::InitializeResponse,
// agent_client_protocol::Error> {         tracing::info!("Received initialize
// request {arguments:?}");
//         Ok(agent_client_protocol::InitializeResponse::new(agent_client_protocol::ProtocolVersion::V1)
//             .agent_info(agent_client_protocol::Implementation::new("aries",
// "0.1.0").title("Aries Agent")))     }
//
//     async fn authenticate(
//         &self,
//         arguments: agent_client_protocol::AuthenticateRequest,
//     ) -> Result<agent_client_protocol::AuthenticateResponse,
// agent_client_protocol::Error> {         tracing::info!("Received authenticate
// request {arguments:?}");
//         Ok(agent_client_protocol::AuthenticateResponse::default())
//     }
//
//     async fn new_session(
//         &self,
//         arguments: agent_client_protocol::NewSessionRequest,
//     ) -> Result<agent_client_protocol::NewSessionResponse,
// agent_client_protocol::Error> {         tracing::info!("Received new session
// request {arguments:?}");         let session_id = self.next_session_id.get();
//         self.next_session_id.set(session_id + 1);
//         Ok(agent_client_protocol::NewSessionResponse::new(session_id.
// to_string()))     }
//
//     async fn prompt(
//         &self,
//         arguments: agent_client_protocol::PromptRequest,
//     ) -> Result<agent_client_protocol::PromptResponse,
// agent_client_protocol::Error> {         tracing::info!("Received prompt
// request {arguments:?}");
//
//         let prompt_text: String = arguments
//             .prompt
//             .iter()
//             .filter_map(|block| {
//                 if let agent_client_protocol::ContentBlock::Text(text) =
// block { Some(text.text.clone()) } else { None }             })
//             .collect::<Vec<_>>()
//             .join("\n");
//
//         let (tx, rx) = oneshot::channel();
//         self.sender
//             .send((
//                 agent_client_protocol::SessionNotification::new(
//                     arguments.session_id.clone(),
//
// agent_client_protocol::SessionUpdate::AgentMessageChunk(agent_client_protocol::ContentChunk::new(
//
// agent_client_protocol::ContentBlock::Text(TextContent::new(format!(
//                             "Aries received: {}",
//                             prompt_text
//                         ))),
//                     )),
//                 ),
//                 tx,
//             ))
//             .map_err(|_| agent_client_protocol::Error::internal_error())?;
//         rx.await.map_err(|_|
// agent_client_protocol::Error::internal_error())?;
//
//         Ok(agent_client_protocol::PromptResponse::new(agent_client_protocol::StopReason::EndTurn))
//     }
//
//     async fn cancel(
//         &self,
//         args: agent_client_protocol::CancelNotification,
//     ) -> Result<(), agent_client_protocol::Error> {
//         tracing::info!("Received cancel request {args:?}");
//         Ok(())
//     }
//
//     async fn load_session(
//         &self,
//         arguments: agent_client_protocol::LoadSessionRequest,
//     ) -> Result<agent_client_protocol::LoadSessionResponse,
// agent_client_protocol::Error> {         tracing::info!("Received load session
// request {arguments:?}");
//         Ok(agent_client_protocol::LoadSessionResponse::new())
//     }
//
//     async fn set_session_mode(
//         &self,
//         args: agent_client_protocol::SetSessionModeRequest,
//     ) -> Result<agent_client_protocol::SetSessionModeResponse,
// agent_client_protocol::Error> {         tracing::info!("Received set session
// mode request {args:?}");
//         Ok(agent_client_protocol::SetSessionModeResponse::default())
//     }
//
//     async fn set_session_config_option(
//         &self,
//         args: agent_client_protocol::SetSessionConfigOptionRequest,
//     ) -> Result<agent_client_protocol::SetSessionConfigOptionResponse,
// agent_client_protocol::Error> {         tracing::info!("Received set session
// config option request {args:?}");         let value:
// agent_client_protocol::SessionConfigValueId = args.value;         let option
// = agent_client_protocol::SessionConfigOption::select(
// args.config_id,             "Example Option",
//             value,
//             vec![
//
// agent_client_protocol::SessionConfigSelectOption::new("option1", "Option 1"),
//
// agent_client_protocol::SessionConfigSelectOption::new("option2", "Option 2"),
//             ],
//         );
//         Ok(agent_client_protocol::SetSessionConfigOptionResponse::new(vec!
// [option]))     }
//
//     async fn ext_method(
//         &self,
//         args: agent_client_protocol::ExtRequest,
//     ) -> Result<agent_client_protocol::ExtResponse,
// agent_client_protocol::Error> {         tracing::info!("Received extension
// method call: method={}, params={:?}", args.method, args.params);
//         Ok(agent_client_protocol::ExtResponse::new(
//             serde_json::value::to_raw_value(&serde_json::json!({"status":
// "ok"}))                 .map_err(|_|
// agent_client_protocol::Error::internal_error())?                 .into(),
//         ))
//     }
//
//     async fn ext_notification(
//         &self,
//         args: agent_client_protocol::ExtNotification,
//     ) -> Result<(), agent_client_protocol::Error> {
//         tracing::info!("Received extension notification: method={},
// params={:?}", args.method, args.params);         Ok(())
//     }
// }
