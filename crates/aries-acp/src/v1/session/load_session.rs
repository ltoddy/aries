use agent_client_protocol::schema::v1::{
    AvailableCommand, AvailableCommandInput, AvailableCommandsUpdate, ContentBlock, ContentChunk,
    LoadSessionRequest, LoadSessionResponse, SessionNotification, SessionUpdate,
    UnstructuredCommandInput,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use aries_session::SharedRegistry;
use itertools::Itertools;
use tracing::{info, instrument};

use super::config::config_options;
use crate::v1::mcp::McpServers;

#[instrument(name = "acp.load_session", skip_all, fields(session_id = %req.session_id))]
pub async fn load_session(
    req: LoadSessionRequest,
    responder: Responder<LoadSessionResponse>,
    cx: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received list sessions request {req:?}");

    let session_id = req.session_id.to_string();
    let mut registry = registry.lock().await;

    let mcp_servers = McpServers(req.mcp_servers);
    let mcp_config = mcp_servers.into();
    let session = match registry.load_session(session_id, mcp_config).await {
        Ok(session) => session,
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };

    let setting = session.setting();
    let greeting = if setting.nickname.is_empty() {
        format!("Welcome back! [session id: {}]", session.id())
    } else {
        format!("Welcome back, {}! [session id: {}]", setting.nickname, session.id())
    };

    let _ = cx.send_notification(SessionNotification::new(
        session.id(),
        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::from(greeting))),
    ));

    let available_commands = session
        .list_available_commands()
        .into_iter()
        .map(|c| {
            AvailableCommand::new(c.name, c.description).input(c.argument_hint.map(|hint| {
                AvailableCommandInput::Unstructured(UnstructuredCommandInput::new(hint))
            }))
        })
        .collect_vec();
    let _ = cx.send_notification(SessionNotification::new(
        session.id(),
        SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(available_commands)),
    ));

    let config_options = config_options(session.setting(), session.mode());
    let resp = LoadSessionResponse::new().config_options(config_options);
    responder.respond(resp)
}
