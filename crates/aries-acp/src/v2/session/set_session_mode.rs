use agent_client_protocol::v1::schema::{SetSessionModeRequest, SetSessionModeResponse};
use agent_client_protocol::v2::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn set_session_mode(
    req: SetSessionModeRequest,
    responder: Responder<SetSessionModeResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received set session mode request (v2): {req:?}");
    responder.respond(SetSessionModeResponse::new())
}
