use agent_client_protocol::schema::v1::{ForkSessionRequest, ForkSessionResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use aries_session::SharedRegistry;
use tracing::info;

use super::config::config_options;
use crate::v1::mcp::McpServers;

pub async fn fork_session(
    req: ForkSessionRequest,
    responder: Responder<ForkSessionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received fork session request {req:?}");
    let session_id = req.session_id.to_string();
    let mcp_servers = McpServers(req.mcp_servers);

    let mut registry = registry.lock().await;
    let session = registry.fork_session(session_id, req.cwd, mcp_servers.into()).await?;

    let config_options = config_options(session.setting(), session.mode());
    let resp = ForkSessionResponse::new(session.id()).config_options(config_options);
    responder.respond(resp)
}
