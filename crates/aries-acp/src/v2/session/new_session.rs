use agent_client_protocol::schema::v2::{
    AvailableCommand, AvailableCommandInput, AvailableCommandsUpdate, ContentBlock, ContentChunk,
    NewSessionRequest, NewSessionResponse, SessionUpdate, TextCommandInput,
    UpdateSessionNotification,
};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use aries_session::{SessionArgs, SharedRegistry};
use itertools::Itertools;
use tracing::info;

use super::config::config_options;
use crate::v2::mcp::McpServers;

pub async fn new_session(
    req: NewSessionRequest,
    responder: Responder<NewSessionResponse>,
    cx: V2ConnectionTo<Client>,
    registry: SharedRegistry,
    args: SessionArgs,
) -> Result<(), Error> {
    info!("Received new session request (v2): {req:?}");

    let mcp_config = McpServers(req.mcp_servers).into();
    let mut registry = registry.lock().await;
    let session = match registry.new_session(req.cwd, mcp_config, args).await {
        Ok(session) => session,
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };

    let setting = session.setting();
    let greeting = if setting.nickname.is_empty() {
        format!("Welcome! [session id: {}]", session.id())
    } else {
        format!("Welcome, {}! [session id: {}]", setting.nickname, session.id())
    };

    let _ = cx.send_notification(UpdateSessionNotification::new(
        session.id(),
        SessionUpdate::AgentMessageChunk(ContentChunk::new(
            ContentBlock::from(greeting),
            "greeting",
        )),
    ));

    let slash_commands = session
        .list_slash_commands()
        .into_iter()
        .map(|c| {
            AvailableCommand::new(c.name, c.description).input(
                c.argument_hint
                    .map(|hint| AvailableCommandInput::Text(TextCommandInput::new(hint))),
            )
        })
        .collect_vec();
    let _ = cx.send_notification(UpdateSessionNotification::new(
        session.id(),
        SessionUpdate::AvailableCommandsUpdate(AvailableCommandsUpdate::new(slash_commands)),
    ));

    let resp = NewSessionResponse::new(session.id())
        .config_options(config_options(session.setting(), session.mode()));
    responder.respond(resp)
}
