use agent_client_protocol::schema::{
    AgentCapabilities, Implementation, InitializeRequest, InitializeResponse, McpCapabilities,
    PromptCapabilities, ProtocolVersion, SessionAdditionalDirectoriesCapabilities,
    SessionCapabilities, SessionListCapabilities,
};
use agent_client_protocol::{Client, ConnectionTo, Responder};
use tracing::info;

pub async fn initialize(
    req: InitializeRequest,
    responder: Responder<InitializeResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), agent_client_protocol::schema::Error> {
    info!("Received initialize request {req:?}");

    let info = Implementation::new("Aries", "0.0.1").title("Aries Agent");
    let capabilities = AgentCapabilities::new()
        .load_session(true)
        .prompt_capabilities(
            PromptCapabilities::new().image(true).audio(true).embedded_context(true),
        )
        .mcp_capabilities(McpCapabilities::new().http(true).sse(true))
        .session_capabilities(
            SessionCapabilities::new()
                .list(SessionListCapabilities::new())
                .additional_directories(SessionAdditionalDirectoriesCapabilities::new()),
        );

    let resp = InitializeResponse::new(ProtocolVersion::LATEST)
        .agent_info(info)
        .agent_capabilities(capabilities);

    responder.respond(resp)
}
