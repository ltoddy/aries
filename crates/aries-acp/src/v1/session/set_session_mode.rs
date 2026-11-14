use agent_client_protocol::v1::schema::{
    Notice, NoticeSeverity, SessionNotification, SessionUpdate, SetSessionModeRequest,
    SetSessionModeResponse,
};
use agent_client_protocol::v1::{ConnectionTo, Responder, schema};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;

pub async fn set_session_mode(
    req: SetSessionModeRequest,
    responder: Responder<SetSessionModeResponse>,
    cx: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    let session_id = req.session_id.to_string();
    let registry = registry.lock().await;
    let Some(mut session) = registry.session(&session_id) else {
        return responder
            .respond_with_error(schema::Error::resource_not_found(Some(session_id)).into());
    };

    let mode = req.mode_id.to_string();
    let _ = match session.set_session_mode(&mode).await {
        Ok(_) => cx.send_notification(SessionNotification::new(
            session.id(),
            SessionUpdate::Notice(Notice::new(NoticeSeverity::Info, "Session mode updated")),
        )),
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };

    let resp = SetSessionModeResponse::new();
    responder.respond(resp)
}
