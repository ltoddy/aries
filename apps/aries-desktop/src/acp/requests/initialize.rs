use std::time::Duration;

use agent_client_protocol::Agent;
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{
    AgentCapabilities, BooleanConfigOptionCapabilities, ClientCapabilities,
    ClientSessionCapabilities, CompactionCapabilities, FileSystemCapabilities, Implementation,
    InitializeRequest, NoticeCapabilities, PlanCapabilities, SessionConfigOptionsCapabilities,
    SubagentCapabilities,
};

use crate::acp::requests::RequestError;

pub async fn request(connection: &ConnectionTo<Agent>) -> Result<AgentCapabilities, RequestError> {
    let response = tokio::time::timeout(
        Duration::from_secs(30),
        connection
            .send_request(
                InitializeRequest::new(ProtocolVersion::V1)
                    .client_capabilities(
                        ClientCapabilities::new()
                            .fs(FileSystemCapabilities::new()
                                .read_text_file(true)
                                .write_text_file(true))
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
                            .subagents(SubagentCapabilities::new())
                            .plan(PlanCapabilities::new()),
                    )
                    .client_info(Implementation::new("aries-desktop", env!("CARGO_PKG_VERSION"))),
            )
            .block_task(),
    )
    .await
    .map_err(|source| RequestError::Timeout { message: "初始化 Agent 超时", source })??;
    if response.protocol_version != ProtocolVersion::V1 {
        return Err(RequestError::UnsupportedVersion);
    }
    Ok(response.agent_capabilities)
}
