use agent_client_protocol::v1::schema::{SetSessionTitleRequest, SetSessionTitleResponse};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn set_session_title(
    req: SetSessionTitleRequest,
    responder: Responder<SetSessionTitleResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received set session title request {req:?}");

    // TODO: Persist the requested session title.
    responder.respond_with_error(
        Error::internal_error().data("session/set_title is not implemented yet"),
    )
}
