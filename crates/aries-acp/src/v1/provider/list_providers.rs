use agent_client_protocol::schema::v1::{
    ListProvidersRequest, ListProvidersResponse, LlmProtocol, ProviderInfo,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn list_providers(
    _req: ListProvidersRequest,
    responder: Responder<ListProvidersResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received list providers request");

    let providers: Vec<ProviderInfo> = vec![
        ProviderInfo::new("azure", vec![LlmProtocol::Azure], false, None),
        ProviderInfo::new("deepseek", vec![LlmProtocol::Other("deepseek".to_owned())], false, None),
        ProviderInfo::new("openai", vec![LlmProtocol::OpenAi], false, None),
    ];

    responder.respond(ListProvidersResponse::new(providers))
}
