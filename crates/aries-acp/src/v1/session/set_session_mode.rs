use agent_client_protocol::schema::v1::{SetSessionModeRequest, SetSessionModeResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};

pub async fn set_session_mode(
    _req: SetSessionModeRequest,
    responder: Responder<SetSessionModeResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    let resp = SetSessionModeResponse::new();
    responder.respond(resp)
}
