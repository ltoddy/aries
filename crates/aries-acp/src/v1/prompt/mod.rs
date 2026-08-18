pub mod elicitation;
pub mod message;
pub mod plan;
pub mod session_update;

use std::collections::HashMap;

use agent_client_protocol::schema::v1::{
    PromptRequest, PromptResponse, SessionNotification, StopReason,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use aries_event::AgentEvent;
use aries_tools::question::AskUserQuestionArgs;
use parking_lot::Mutex;
use rig::completion::Message;
use rig::message::ToolCall;
use tracing::{info, instrument, warn};

use self::elicitation::Elicitation;
use self::message::UserMessage;
use self::session_update::SessionUpdates;
use super::SharedRegistry;

#[instrument(name = "acp.prompt", skip_all, fields(session_id = %req.session_id))]
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
        match registry.session(&session_id) {
            Some(session) => session,
            None => {
                return responder.respond_with_error(Error::resource_not_found(Some(session_id)));
            },
        }
    };

    let user_message = UserMessage::from(req.prompt);
    let prompt = user_message.into();

    let tool_names = Mutex::new(HashMap::<String, ToolCall>::new());
    let pending = Mutex::new(None::<AskUserQuestionArgs>);

    let callback = async |event: AgentEvent| match event {
        AgentEvent::AwaitingUserInput { args } => {
            match serde_json::from_value::<AskUserQuestionArgs>(args.clone()) {
                Ok(question) => *pending.lock() = Some(question),
                Err(err) => warn!("failed to parse AskUserQuestion args: {err}"),
            }
        },
        _ => {
            SessionUpdates::new(event, &tool_names).into_iter().for_each(|u| {
                let _ = cx.send_notification(SessionNotification::new(session_id.clone(), u));
            });
        },
    };

    let mut prompt = prompt;
    loop {
        match session.prompt(prompt, callback).await {
            Ok(_) => {
                let question = pending.lock().take();
                match question {
                    Some(question) => {
                        let answer = Elicitation::new(cx.clone(), &session_id).ask(&question).await;
                        prompt = Message::user(answer.to_input(&question));
                    },
                    None => break,
                }
            },
            Err(err) => return responder.respond_with_internal_error(err.to_string()),
        }
    }

    let mut registry = registry.lock().await;
    registry.putback_session(session);
    responder.respond(PromptResponse::new(StopReason::EndTurn))
}
