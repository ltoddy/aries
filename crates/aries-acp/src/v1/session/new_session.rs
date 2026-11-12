use agent_client_protocol::v1::schema::{
    AvailableCommand, AvailableCommandInput, AvailableCommandsUpdate, NewSessionRequest,
    NewSessionResponse, SessionInfoUpdate, SessionNotification, SessionUpdate,
    UnstructuredCommandInput,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::session::{DEFAULT_AGENT_NAME, SharedRegistry};
use itertools::Itertools;
use tracing::info;

use crate::v1::mcp::McpServers;
use crate::v1::session::config::{config_options, modes};

pub async fn new_session(
    req: NewSessionRequest,
    responder: Responder<NewSessionResponse>,
    cx: ConnectionTo<Client>,
    registry: &SharedRegistry,
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

    let modes = match modes(DEFAULT_AGENT_NAME, session.agents()) {
        Ok(modes) => modes,
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };
    let config_options = config_options(
        session.setting(),
        session.mode(),
        session.agents(),
        session.thought_level(),
    );
    let resp = NewSessionResponse::new(session.id()).modes(modes).config_options(config_options);
    responder.respond(resp)
}
