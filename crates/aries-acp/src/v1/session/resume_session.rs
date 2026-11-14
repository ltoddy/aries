use agent_client_protocol::v1::schema::{
    AvailableCommand, AvailableCommandInput, AvailableCommandsUpdate, ResumeSessionRequest,
    ResumeSessionResponse, SessionNotification, SessionUpdate, UnstructuredCommandInput,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;
use itertools::Itertools;
use tracing::{info, instrument};

use crate::v1::mcp::McpServers;
use crate::v1::session::config::{config_options, modes};

#[instrument(name = "acp.resume_session", skip_all, fields(session_id = %req.session_id))]
pub async fn resume_session(
    req: ResumeSessionRequest,
    responder: Responder<ResumeSessionResponse>,
    cx: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received resume session request {req:?}");

    let session_id = req.session_id.to_string();
    let mcp_config = McpServers::new(req.mcp_servers).into();
    let mut registry = registry.lock().await;
    let session = match registry.load_session(session_id, mcp_config).await {
        Ok(session) => session,
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };
    drop(registry);

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

    let modes = match modes(session.mode(), session.agents()) {
        Ok(modes) => modes,
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };
    let config_options = config_options(
        session.setting(),
        session.mode(),
        session.agents(),
        session.thought_level(),
    );
    let resp = ResumeSessionResponse::new().modes(modes).config_options(config_options);
    responder.respond(resp)
}
