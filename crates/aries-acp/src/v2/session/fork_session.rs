use agent_client_protocol::schema::v2::{ForkSessionRequest, ForkSessionResponse};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use aries_session::SharedRegistry;
use tracing::info;

use super::config::config_options;
use crate::v2::mcp::McpServers;

pub async fn fork_session(
    req: ForkSessionRequest,
    responder: Responder<ForkSessionResponse>,
    _cx: V2ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received fork session request (v2): {req:?}");

    let session_id = req.session_id.to_string();
    let mcp_config = McpServers(req.mcp_servers).into();
    let mut registry = registry.lock().await;
    let session = registry.fork_session(session_id, req.cwd, mcp_config).await?;

    let resp = ForkSessionResponse::new(session.id())
        .config_options(config_options(session.setting(), session.mode()));
    responder.respond(resp)
}
