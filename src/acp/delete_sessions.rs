use std::collections::HashSet;
use std::time::Duration;

use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{DeleteSessionRequest, SessionId};
use agent_client_protocol::{Agent, Error};

#[derive(Debug, Default)]
pub struct DeletionReport {
    deleted: Vec<SessionId>,
    failed: Vec<(SessionId, Error)>,
}

impl DeletionReport {
    pub fn into_parts(self) -> (Vec<SessionId>, Vec<(SessionId, Error)>) {
        (self.deleted, self.failed)
    }

    pub fn rejected(ids: Vec<SessionId>, message: &str) -> Self {
        Self {
            deleted: Vec::new(),
            failed: ids.into_iter().map(|id| (id, Error::internal_error().data(message))).collect(),
        }
    }
}

pub async fn request(connection: &ConnectionTo<Agent>, ids: Vec<SessionId>) -> DeletionReport {
    let mut result = DeletionReport::default();
    let mut seen = HashSet::new();
    for id in ids.into_iter().filter(|id| seen.insert(id.clone())) {
        let response = tokio::time::timeout(
            Duration::from_secs(30),
            connection.send_request(DeleteSessionRequest::new(id.clone())).block_task(),
        )
        .await
        .map_err(|_| Error::internal_error().data("删除会话超时，刷新列表以确认结果"))
        .and_then(|response| response);
        match response {
            Ok(_) => result.deleted.push(id),
            Err(error) => result.failed.push((id, error)),
        }
    }
    result
}
