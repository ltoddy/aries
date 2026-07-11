pub mod message;
pub mod plan;
pub mod session_update;

use std::collections::HashMap;

use agent_client_protocol::schema::v1::{
    PromptRequest, PromptResponse, SessionNotification, StopReason,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use aries_agent::event::AgentEvent;
use parking_lot::Mutex;
use rig_core::message::ToolCall;
use tracing::{info, info_span};

use self::message::UserMessage;
use self::session_update::SessionUpdates;
use super::SharedRegistry;

pub async fn prompt(
    req: PromptRequest,
    responder: Responder<PromptResponse>,
    cx: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received prompt request {req:?}");

    let session_id = req.session_id.to_string();
    let mut session = {
        let registry = registry.lock().await;
        match registry.get_session(&session_id) {
            Some(session) => session,
            None => {
                return responder.respond_with_error(Error::resource_not_found(Some(session_id)));
            },
        }
    };

    let prompt = UserMessage::from(req.prompt);
    let tool_names = Mutex::new(HashMap::<String, ToolCall>::new());

    let callback = async |event: AgentEvent| {
        SessionUpdates::new(event, &tool_names).into_iter().for_each(|u| {
            let _ = cx.send_notification(SessionNotification::new(session_id.clone(), u));
        });
    };

    let span = info_span!("prompt", session_id = %session_id);
    let _enter = span.enter();

    match session.prompt(prompt, Some(callback)).await {
        Ok(_) => {
            let mut registry = registry.lock().await;
            registry.putback_session(session);
            responder.respond(PromptResponse::new(StopReason::EndTurn))
        },
        Err(err) => responder.respond_with_internal_error(err.to_string()),
    }
}
