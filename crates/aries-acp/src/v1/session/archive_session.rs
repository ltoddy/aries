use agent_client_protocol::v1::schema::{ArchiveSessionRequest, ArchiveSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn archive_session(
    req: ArchiveSessionRequest,
    responder: Responder<ArchiveSessionResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received archive session request {req:?}");

    let resp = ArchiveSessionResponse::new();
    responder.respond(resp)
}
