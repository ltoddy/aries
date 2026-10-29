use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, Implementation, InitializeRequest, InitializeResponse, McpCapabilities,
    PromptCapabilities, ProvidersCapabilities, SessionAdditionalDirectoriesCapabilities,
    SessionCapabilities, SessionCloseCapabilities, SessionDeleteCapabilities,
    SessionForkCapabilities, SessionListCapabilities, SessionResumeCapabilities,
    SessionSystemPromptCapabilities,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use tracing::info;

pub async fn initialize(
    req: InitializeRequest,
    responder: Responder<InitializeResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received initialize request {req:?}");

    let info = Implementation::new("Aries", "0.0.1").title("Aries Agent");
    let capabilities = AgentCapabilities::new()
        .load_session(true)
        .prompt_capabilities(
            PromptCapabilities::new().image(true).audio(true).embedded_context(true),
        )
        .mcp_capabilities(McpCapabilities::new().http(true).sse(true))
        .providers(ProvidersCapabilities::new())
        .session_capabilities(
            SessionCapabilities::new()
                .list(SessionListCapabilities::new())
                .delete(SessionDeleteCapabilities::new())
                .additional_directories(SessionAdditionalDirectoriesCapabilities::new())
                .system_prompt(SessionSystemPromptCapabilities::new())
                .fork(SessionForkCapabilities::new())
                .resume(SessionResumeCapabilities::new())
                .close(SessionCloseCapabilities::new()),
        );

    let resp = InitializeResponse::new(ProtocolVersion::V1)
        .agent_info(info)
        .agent_capabilities(capabilities);

    responder.respond(resp)
}
