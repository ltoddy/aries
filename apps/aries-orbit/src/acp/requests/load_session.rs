use std::time::Duration;

use agent_client_protocol::Agent;
use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{LoadSessionRequest, LoadSessionResponse};

use crate::acp::requests::RequestError;

pub async fn request(
    connection: &ConnectionTo<Agent>,
    request: LoadSessionRequest,
) -> Result<LoadSessionResponse, RequestError> {
    if !request.cwd.is_absolute() {
        return Err(RequestError::InvalidCwd);
    }
    tokio::time::timeout(Duration::from_secs(30), connection.send_request(request).block_task())
        .await
        .map_err(|source| RequestError::Timeout { message: "加载会话超时", source })?
        .map_err(RequestError::from)
}
