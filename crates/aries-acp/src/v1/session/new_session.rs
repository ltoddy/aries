use agent_client_protocol::v1::schema::{
    AvailableCommand, AvailableCommandInput, AvailableCommandsUpdate, ContentBlock, ContentChunk,
    NewSessionRequest, NewSessionResponse, SessionInfoUpdate, SessionNotification, SessionUpdate,
    UnstructuredCommandInput,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::SharedRegistry;
use itertools::Itertools;
use tracing::info;

use super::config::{config_options, modes};
use crate::v1::mcp::McpServers;

pub async fn new_session(
    req: NewSessionRequest,
    responder: Responder<NewSessionResponse>,
    cx: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received new session request {req:?}");

    let mcp_servers = McpServers(req.mcp_servers);
    let mcp_config = mcp_servers.into();
    let mut registry = registry.lock().await;
    let session = match registry.new_session(req.cwd, mcp_config).await {
        Ok(session) => session,
        Err(err) => {
            return responder.respond_with_internal_error(err.to_string());
        },
    };

    let greeting = format!("Welcome! [session id: {}]", session.id());
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

    let _ = cx.send_notification(SessionNotification::new(
        session.id(),
        SessionUpdate::SessionInfoUpdate(SessionInfoUpdate::new()),
    ));

    let modes = modes("build", session.agents());
    let config_options =
        config_options(session.setting(), session.mode().to_string(), session.agents());
    let resp = NewSessionResponse::new(session.id()).modes(modes).config_options(config_options);
    responder.respond(resp)
}
