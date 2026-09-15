use agent_client_protocol::schema::v1::{SetSessionModeRequest, SetSessionModeResponse};
use agent_client_protocol::{Client, Error, Responder, V2ConnectionTo};
use tracing::info;

pub async fn set_session_mode(
    req: SetSessionModeRequest,
    responder: Responder<SetSessionModeResponse>,
    _cx: V2ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received set session mode request (v2): {req:?}");
    responder.respond(SetSessionModeResponse::new())
}
