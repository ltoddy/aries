use agent_client_protocol::v1::schema::{SetSessionModeRequest, SetSessionModeResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};

pub async fn set_session_mode(
    _req: SetSessionModeRequest,
    responder: Responder<SetSessionModeResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    let resp = SetSessionModeResponse::new();
    responder.respond(resp)
}
