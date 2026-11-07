use agent_client_protocol::v1::schema::{UnarchiveSessionRequest, UnarchiveSessionResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn unarchive_session(
    req: UnarchiveSessionRequest,
    responder: Responder<UnarchiveSessionResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received unarchive session request {req:?}");

    // TODO: Unarchive the requested session.
    responder.respond_with_error(
        Error::internal_error().data("session/unarchive is not implemented yet"),
    )
}
