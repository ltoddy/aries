use std::time::Duration;

use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{LoadSessionRequest, LoadSessionResponse};
use agent_client_protocol::{Agent, Error};

pub async fn request(
    connection: &ConnectionTo<Agent>,
    request: LoadSessionRequest,
) -> Result<LoadSessionResponse, Error> {
    if !request.cwd.is_absolute() {
        return Err(Error::invalid_params().data("会话工作目录必须是绝对路径"));
    }
    tokio::time::timeout(Duration::from_secs(30), connection.send_request(request).block_task())
        .await
        .map_err(|_| Error::internal_error().data("加载会话超时"))?
}
