use agent_client_protocol::v1::schema::{
    AvailableCommand, AvailableCommandInput, AvailableCommandsUpdate, LoadSessionRequest,
    LoadSessionResponse, SessionNotification, SessionUpdate, UnstructuredCommandInput,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;
use aries_runtime::session::DEFAULT_AGENT_NAME;
use itertools::Itertools;
use tracing::{info, instrument};

use crate::v1::mcp::McpServers;
use crate::v1::prompt::session_update::SessionUpdates;
use crate::v1::session::config::{config_options, modes};

#[instrument(name = "acp.load_session", skip_all, fields(session_id = %req.session_id))]
pub async fn load_session(
    req: LoadSessionRequest,
    responder: Responder<LoadSessionResponse>,
    cx: ConnectionTo<Client>,
    registry: &SharedRegistry,
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
    drop(registry);

    let session_id = session.id();
    let messages = match session.history().await {
        Ok(messages) => messages,
        Err(err) => {
            return responder.respond_with_internal_error(format!(
                "failed to load history for session `{session_id}`: {err}"
            ));
        },
    };
    SessionUpdates::from_history(messages).into_iter().for_each(|update| {
        let _ = cx.send_notification(SessionNotification::new(session_id.clone(), update));
    });

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
        session_id,
        SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(available_commands)),
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
    let resp = LoadSessionResponse::new().modes(modes).config_options(config_options);
    responder.respond(resp)
}
