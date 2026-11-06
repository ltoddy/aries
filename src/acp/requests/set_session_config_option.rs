use std::time::Duration;

use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{SessionConfigOption, SetSessionConfigOptionRequest};
use agent_client_protocol::{Agent, Error};

pub async fn request(
    connection: &ConnectionTo<Agent>,
    request: SetSessionConfigOptionRequest,
) -> Result<Vec<SessionConfigOption>, Error> {
    tokio::time::timeout(Duration::from_secs(30), connection.send_request(request).block_task())
        .await
        .map_err(|_| Error::internal_error().data("更新会话配置超时"))?
        .map(|response| response.config_options)
}
