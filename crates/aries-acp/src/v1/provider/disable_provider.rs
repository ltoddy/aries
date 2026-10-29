use agent_client_protocol::schema::v1::{DisableProviderRequest, DisableProviderResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn disable_provider(
    req: DisableProviderRequest,
    responder: Responder<DisableProviderResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received disable provider request: {req:?}");
    responder.respond(DisableProviderResponse::new())
}
