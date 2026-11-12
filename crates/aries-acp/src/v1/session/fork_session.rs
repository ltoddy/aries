use agent_client_protocol::v1::schema::{ForkSessionRequest, ForkSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::{DEFAULT_AGENT_NAME, SharedRegistry};
use tracing::info;

use crate::v1::mcp::McpServers;
use crate::v1::session::config::{config_options, modes};

pub async fn fork_session(
    req: ForkSessionRequest,
    responder: Responder<ForkSessionResponse>,
    _: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received fork session request {req:?}");
    let session_id = req.session_id.to_string();
    let mcp_servers = McpServers(req.mcp_servers);

    let mut registry = registry.lock().await;
    let session = registry
        .fork_session(session_id, req.cwd, mcp_servers.into())
        .await
        .map_err(|e| Error::internal_error().data(e.to_string()))?;

    let modes = modes(DEFAULT_AGENT_NAME, session.agents())
        .map_err(|e| Error::internal_error().data(e.to_string()))?;
    let config_options = config_options(
        session.setting(),
        session.mode(),
        session.agents(),
        session.thought_level(),
    );
    let resp = ForkSessionResponse::new(session.id()).modes(modes).config_options(config_options);
    responder.respond(resp)
}
