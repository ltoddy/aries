use agent_client_protocol::v2::schema::{ForkSessionRequest, ForkSessionResponse};
use agent_client_protocol::v2::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
use tracing::info;

use super::config::config_options;
use crate::v2::mcp::McpServers;

pub async fn fork_session(
    req: ForkSessionRequest,
    responder: Responder<ForkSessionResponse>,
    _cx: ConnectionTo<Client>,
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
