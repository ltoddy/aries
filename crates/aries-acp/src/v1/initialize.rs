use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::schema::{
    AgentAuthCapabilities, AgentCapabilities, AuthMethod, AuthMethodTerminal,
    AuthStatusCapabilities, Implementation, InitializeRequest, InitializeResponse,
    LogoutCapabilities, McpCapabilities, PromptCapabilities, ProvidersCapabilities,
    SessionAdditionalDirectoriesCapabilities, SessionCapabilities, SessionCloseCapabilities,
    SessionDeleteCapabilities, SessionForkCapabilities, SessionListCapabilities,
    SessionResumeCapabilities, SessionSystemPromptCapabilities,
};
use agent_client_protocol::v1::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn initialize(
    req: InitializeRequest,
    responder: Responder<InitializeResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received initialize request {req:?}");

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
        )
        .auth(
            AgentAuthCapabilities::new()
                .status(AuthStatusCapabilities::new())
                .logout(LogoutCapabilities::new()),
        );
    let auth_methods = vec![AuthMethod::Terminal(
        AuthMethodTerminal::new("aries-setup", "Set up Aries")
            .description("Configure your model provider and API key in a terminal")
            .args(vec!["--setup".into()]),
    )];
    let info = Implementation::new("Aries", "0.0.1").title("Aries Agent");

    let resp = InitializeResponse::new(ProtocolVersion::V1)
        .agent_capabilities(capabilities)
        .auth_methods(auth_methods)
        .agent_info(info);

    responder.respond(resp)
}
