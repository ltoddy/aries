use agent_client_protocol::v1::schema::{SetSessionTitleRequest, SetSessionTitleResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn set_session_title(
    req: SetSessionTitleRequest,
    responder: Responder<SetSessionTitleResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received set session title request {req:?}");

    let resp = SetSessionTitleResponse::new(req.title);
    responder.respond(resp)
}
