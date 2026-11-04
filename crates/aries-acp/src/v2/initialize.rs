use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v2::schema::{
    AgentAuthCapabilities, AgentCapabilities, Implementation, InitializeRequest,
    InitializeResponse, McpAcpCapabilities, McpCapabilities, McpHttpCapabilities,
    McpStdioCapabilities, PromptAudioCapabilities, PromptCapabilities,
    PromptEmbeddedContextCapabilities, PromptImageCapabilities, ProvidersCapabilities,
    SessionAdditionalDirectoriesCapabilities, SessionCapabilities, SessionDeleteCapabilities,
    SessionForkCapabilities,
};
use agent_client_protocol::v2::{ConnectionTo, Responder};
use agent_client_protocol::{Client, Error};
use tracing::info;

pub async fn initialize(
    req: InitializeRequest,
    responder: Responder<InitializeResponse>,
    _cx: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received initialize request (v2): {req:?}");

    let resp = InitializeResponse::new(
        ProtocolVersion::V2,
        Implementation::new("Aries", "0.0.1").title("Aries Agent"),
    )
    .capabilities(
        AgentCapabilities::new()
            .session(
                SessionCapabilities::new()
                    .prompt(
                        PromptCapabilities::new()
                            .image(PromptImageCapabilities::new())
                            .audio(PromptAudioCapabilities::new())
                            .embedded_context(PromptEmbeddedContextCapabilities::new()),
                    )
                    .mcp(
                        McpCapabilities::new()
                            .stdio(McpStdioCapabilities::new())
                            .http(McpHttpCapabilities::new())
                            .acp(McpAcpCapabilities::new()),
                    )
                    .delete(SessionDeleteCapabilities::new())
                    .additional_directories(SessionAdditionalDirectoriesCapabilities::new())
                    .fork(SessionForkCapabilities::new()),
            )
            .auth(AgentAuthCapabilities::new())
            .providers(ProvidersCapabilities::new()),
    )
    .auth_methods(vec![]);

    responder.respond(resp)
}
