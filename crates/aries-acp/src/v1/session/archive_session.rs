use agent_client_protocol::v1::schema::{ArchiveSessionRequest, ArchiveSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn archive_session(
    req: ArchiveSessionRequest,
    responder: Responder<ArchiveSessionResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received archive session request {req:?}");

    // TODO: Archive the requested session.
    responder
        .respond_with_error(Error::internal_error().data("session/archive is not implemented yet"))
}
