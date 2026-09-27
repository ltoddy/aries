mod message;
mod plan;
mod session_update;

use std::collections::HashMap;

use agent_client_protocol::schema::v2::{
    IdleStateUpdate, PromptRequest, PromptResponse, StateUpdate, StopReason,
    UpdateSessionNotification,
};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use aries_event::AgentEvent;
use aries_session::{PromptOutcome, SharedRegistry};
use parking_lot::Mutex;
use rig::completion::Message;
use rig::message::ToolCall;
use tracing::info;

use self::message::UserMessage;
use self::session_update::SessionUpdates;

pub async fn prompt(
    req: PromptRequest,
    responder: Responder<PromptResponse>,
    cx: V2ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received prompt request (v2): {req:?}");

    let session_id = req.session_id.to_string();
    let mut session = {
        let registry = registry.lock().await;
        match registry.session(&session_id) {
            Some(session) => session,
            None => {
                return responder.respond_with_error(Error::resource_not_found(Some(session_id)));
            },
        }
    };

    let tool_calls = Mutex::new(HashMap::<String, ToolCall>::new());
    let callback = async |event: AgentEvent| {
        SessionUpdates::new(event, &tool_calls).into_iter().for_each(|u| {
            let _ = cx.send_notification(UpdateSessionNotification::new(session_id.clone(), u));
        });
    };

    let user_message = UserMessage::from(req.prompt);
    let prompt: Message = user_message.into();
    let result = match session.prompt(prompt, callback).await {
        Ok(result) => result,
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };
    let (message_id, stop_reason) = match result {
        PromptOutcome::Completed(message_id) => (message_id, StopReason::EndTurn),
        PromptOutcome::Cancelled(message_id) => (message_id, StopReason::Cancelled),
    };

    let _ = cx.send_notification(UpdateSessionNotification::new(
        session_id.clone(),
        agent_client_protocol::schema::v2::SessionUpdate::StateUpdate(StateUpdate::Idle(
            IdleStateUpdate::new().stop_reason(stop_reason),
        )),
    ));

    let mut registry = registry.lock().await;
    registry.putback_session(session);
    responder.respond(PromptResponse::new(message_id))
}
