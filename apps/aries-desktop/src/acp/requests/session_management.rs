use std::time::Duration;

use agent_client_protocol::Agent;
use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{
    ArchiveSessionRequest, ForkSessionRequest, ForkSessionResponse, SessionId,
    SetSessionTitleRequest, UnarchiveSessionRequest,
};

use crate::acp::requests::RequestError;

pub async fn set_title(
    connection: &ConnectionTo<Agent>,
    request: SetSessionTitleRequest,
) -> Result<String, RequestError> {
    tokio::time::timeout(Duration::from_secs(30), connection.send_request(request).block_task())
        .await
        .map_err(|source| RequestError::timeout("重命名会话超时", source))?
        .map(|response| response.title)
        .map_err(RequestError::from)
}

pub async fn archive(
    connection: &ConnectionTo<Agent>,
    session_id: SessionId,
) -> Result<(), RequestError> {
    tokio::time::timeout(
        Duration::from_secs(30),
        connection.send_request(ArchiveSessionRequest::new(session_id)).block_task(),
    )
    .await
    .map_err(|source| RequestError::timeout("归档会话超时", source))?
    .map(|_| ())
    .map_err(RequestError::from)
}

pub async fn unarchive(
    connection: &ConnectionTo<Agent>,
    session_id: SessionId,
) -> Result<(), RequestError> {
    tokio::time::timeout(
        Duration::from_secs(30),
        connection.send_request(UnarchiveSessionRequest::new(session_id)).block_task(),
    )
    .await
    .map_err(|source| RequestError::timeout("恢复会话超时", source))?
    .map(|_| ())
    .map_err(RequestError::from)
}

pub async fn fork(
    connection: &ConnectionTo<Agent>,
    request: ForkSessionRequest,
) -> Result<ForkSessionResponse, RequestError> {
    tokio::time::timeout(Duration::from_secs(30), connection.send_request(request).block_task())
        .await
        .map_err(|source| RequestError::timeout("复制会话超时", source))?
        .map_err(RequestError::from)
}
