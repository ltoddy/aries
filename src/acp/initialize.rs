use std::time::Duration;

use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, BooleanConfigOptionCapabilities, ClientCapabilities,
    ClientSessionCapabilities, CompactionCapabilities, Implementation, InitializeRequest,
    NoticeCapabilities, PlanCapabilities, SessionConfigOptionsCapabilities, SubagentCapabilities,
};
use agent_client_protocol::{Agent, ConnectionTo, Error};

pub async fn request(connection: &ConnectionTo<Agent>) -> Result<AgentCapabilities, Error> {
    let response = tokio::time::timeout(
        Duration::from_secs(30),
        connection
            .send_request(
                InitializeRequest::new(ProtocolVersion::V1)
                    .client_capabilities(
                        ClientCapabilities::new()
                            .terminal(true)
                            .session(
                                ClientSessionCapabilities::new()
                                    .config_options(
                                        SessionConfigOptionsCapabilities::new()
                                            .boolean(BooleanConfigOptionCapabilities::new()),
                                    )
                                    .notices(NoticeCapabilities::new())
                                    .compaction(CompactionCapabilities::new()),
                            )
                            .plan(PlanCapabilities::new())
                            .subagents(SubagentCapabilities::new()),
                    )
                    .client_info(Implementation::new("aries-orbit", env!("CARGO_PKG_VERSION"))),
            )
            .block_task(),
    )
    .await
    .map_err(|_| Error::internal_error().data("初始化 Agent 超时"))??;
    if response.protocol_version != ProtocolVersion::V1 {
        return Err(Error::internal_error().data("Agent 未协商 ACP v1"));
    }
    Ok(response.agent_capabilities)
}
