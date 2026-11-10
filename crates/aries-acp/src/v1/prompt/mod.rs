pub mod message;
pub mod plan;
pub mod session_update;

use agent_client_protocol::v1::schema::{
    self, PromptRequest, PromptResponse, SessionInfoUpdate, SessionNotification, SessionUpdate,
    StopReason,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use aries_communicate::AgentEvent;
use aries_runtime::session::{PromptOutcome, SharedRegistry};
use tracing::{info, instrument};

use self::message::UserMessage;
use self::session_update::SessionUpdates;

#[instrument(name = "acp.prompt", skip_all, fields(session_id = %req.session_id))]
pub async fn prompt(
    req: PromptRequest,
    responder: Responder<PromptResponse>,
    cx: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!(content_blocks = req.prompt.len(), "Received prompt request");

    let session_id = req.session_id.to_string();
    let prompt = UserMessage::from(req.prompt);
    let callback = async |event: AgentEvent| {
        SessionUpdates::new(event).into_iter().for_each(|u| {
            let _ = cx.send_notification(SessionNotification::new(session_id.clone(), u));
        });
    };

    let mut session = {
        let registry = registry.lock().await;
        match registry.session(&session_id) {
            Some(session) => session,
            None => {
                return responder.respond_with_error(
                    schema::Error::resource_not_found(Some(session_id)).into(),
                );
            },
        }
    };

    let result = session.prompt(prompt, callback).await;
    {
        let mut registry = registry.lock().await;
        registry.putback_session(session);
    }
    let stop_reason = match result {
        Ok(PromptOutcome::Completed(_)) => StopReason::EndTurn,
        Ok(PromptOutcome::Cancelled(_)) => StopReason::Cancelled,
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };

    let _ = cx.send_notification(SessionNotification::new(
        session_id,
        SessionUpdate::SessionInfoUpdate(SessionInfoUpdate::new()),
    ));

    responder.respond(PromptResponse::new(stop_reason))
}
