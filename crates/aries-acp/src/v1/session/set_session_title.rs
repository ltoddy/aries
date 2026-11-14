use agent_client_protocol::v1::schema::{
    Notice, NoticeSeverity, SessionNotification, SessionUpdate, SetSessionTitleRequest,
    SetSessionTitleResponse,
};
use agent_client_protocol::v1::{ConnectionTo, Responder, schema};
use agent_client_protocol::{Client, Error};
use aries_runtime::registry::SharedRegistry;
use tracing::info;

pub async fn set_session_title(
    req: SetSessionTitleRequest,
    responder: Responder<SetSessionTitleResponse>,
    cx: ConnectionTo<Client>,
    registry: &SharedRegistry,
) -> Result<(), Error> {
    info!("Received set session title request {req:?}");

    let session_id = req.session_id.to_string();
    let registry = registry.lock().await;
    let Some(mut session) = registry.session(&session_id) else {
        return responder
            .respond_with_error(schema::Error::resource_not_found(Some(session_id)).into());
    };

    let _ = match session.set_session_title(&req.title).await {
        Ok(_) => cx.send_notification(SessionNotification::new(
            session.id(),
            SessionUpdate::Notice(Notice::new(NoticeSeverity::Info, "Session title updated")),
        )),
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };

    let resp = SetSessionTitleResponse::new(req.title);
    responder.respond(resp)
}
