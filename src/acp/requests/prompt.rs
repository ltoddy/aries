use std::ops::ControlFlow;
use std::time::Duration;

use agent_client_protocol::v1::schema::{
    CancelNotification, ContentBlock, PromptRequest, SessionId,
};

use crate::acp::connection::{self, AgentConnection};
use crate::acp::requests::RequestError;
use crate::acp::{Command, Event};

pub async fn request(
    connection: &AgentConnection,
    session_id: SessionId,
    text: String,
) -> Result<ControlFlow<()>, RequestError> {
    if text.trim().is_empty() {
        return Ok(ControlFlow::Continue(()));
    }
    connection.set_cancelled(false);
    let response = connection
        .client()
        .send_request(PromptRequest::new(session_id.clone(), vec![ContentBlock::from(text)]))
        .block_task();
    tokio::pin!(response);
    let deadline = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(deadline);
    let mut cancel = Some(CancelNotification::new(session_id));
    let result = loop {
        tokio::select! {
            result = &mut response => break result,
            command = connection.commands().recv() => match command {
                Ok(Command::Cancel) => {
                    if let Some(notification) = cancel.take() {
                        connection.set_cancelled(true);
                        connection.client().send_notification(notification)?;
                        deadline.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(10));
                    }
                }
                Ok(command) => connection.handle_busy_command(command).await,
                Err(_) => return Ok(ControlFlow::Break(())),
            },
            _ = &mut deadline, if cancel.is_none() => return Err(RequestError::CancellationTimeout),
            _ = connection.client().incoming_closed() => return Err(connection::closed().into()),
        }
    };
    connection.emit(Event::Finished(result.map(|response| response.stop_reason))).await;
    Ok(ControlFlow::Continue(()))
}
