use agent_client_protocol::schema::v1::{
    DisableProviderRequest, DisableProviderResponse, ListProvidersRequest, ListProvidersResponse,
    SetProviderRequest, SetProviderResponse,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn list_providers(
    req: ListProvidersRequest,
    responder: Responder<ListProvidersResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received list providers request: {req:?}");

    responder.respond(ListProvidersResponse::new(vec![]))
}

pub async fn set_provider(
    req: SetProviderRequest,
    responder: Responder<SetProviderResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received set provider request: {req:?}");
    responder.respond(SetProviderResponse::new())
}

pub async fn disable_provider(
    req: DisableProviderRequest,
    responder: Responder<DisableProviderResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received disable provider request: {req:?}");
    responder.respond(DisableProviderResponse::new())
}
