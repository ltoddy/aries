use agent_client_protocol::schema::v1::{LlmProtocol, SetProviderRequest, SetProviderResponse};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn set_provider(
    req: SetProviderRequest,
    _responder: Responder<SetProviderResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received set provider request");

    match req.api_type {
        LlmProtocol::OpenAi => {},
        LlmProtocol::Azure => {},
        _ => {},
    }

    Ok(())
}
