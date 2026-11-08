use std::collections::HashSet;
use std::time::Duration;

use agent_client_protocol::v1::ConnectionTo;
use agent_client_protocol::v1::schema::{ListSessionsRequest, SessionInfo};
use agent_client_protocol::{Agent, Error};
use async_channel::Sender;

use crate::acp::Event;
use crate::acp::requests::RequestError;

pub fn request(connection: &ConnectionTo<Agent>, events: &Sender<Event>) -> Result<(), Error> {
    let client = connection.clone();
    let events = events.clone();
    connection.spawn(async move {
        let result = fetch(&client).await.map_err(Error::from);
        let _ = events.send(Event::Sessions(result)).await;
        Ok(())
    })
}

async fn fetch(connection: &ConnectionTo<Agent>) -> Result<Vec<SessionInfo>, RequestError> {
    let mut sessions = Vec::new();
    let mut cursor = None;
    let mut cursors = HashSet::new();
    loop {
        let response = tokio::time::timeout(
            Duration::from_secs(30),
            connection.send_request(ListSessionsRequest::new().cursor(cursor)).block_task(),
        )
        .await
        .map_err(|source| RequestError::Timeout {
            message: "获取会话列表超时", source
        })??;
        sessions.extend(response.sessions);
        let Some(next) = response.next_cursor else {
            return Ok(sessions);
        };
        if !cursors.insert(next.clone()) {
            return Err(RequestError::RepeatedCursor);
        }
        cursor = Some(next);
    }
}
