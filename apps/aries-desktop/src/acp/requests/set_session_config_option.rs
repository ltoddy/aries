use std::time::Duration;

use agent_client_protocol::Agent;
use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{SessionConfigOption, SetSessionConfigOptionRequest};

use crate::acp::requests::RequestError;

pub async fn request(
    connection: &ConnectionTo<Agent>,
    request: SetSessionConfigOptionRequest,
) -> Result<Vec<SessionConfigOption>, RequestError> {
    tokio::time::timeout(Duration::from_secs(30), connection.send_request(request).block_task())
        .await
        .map_err(|source| RequestError::Timeout { message: "更新会话配置超时", source })?
        .map(|response| response.config_options)
        .map_err(RequestError::from)
}
