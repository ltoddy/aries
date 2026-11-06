use std::ops::ControlFlow;
use std::time::Duration;

use agent_client_protocol::Error;
use agent_client_protocol::v1::schema::{CancelNotification, ContentBlock, PromptRequest};

use crate::acp::connection::{self, AgentConnection};
use crate::acp::requests::new_session;
use crate::acp::{Command, Event, PromptTarget};

pub async fn request(
    connection: &AgentConnection,
    target: PromptTarget,
    text: String,
) -> Result<ControlFlow<()>, Error> {
    if text.trim().is_empty() {
        return Ok(ControlFlow::Continue(()));
    }
    connection.set_cancelled(false);
    let session_id = match target {
        PromptTarget::Existing(id) => id,
        PromptTarget::New(cwd) => {
            let Some(response) = new_session::request(connection, cwd).await? else {
                return Ok(ControlFlow::Break(()));
            };
            let id = response.session_id.clone();
            connection.emit(Event::Ready(Box::new(response))).await;
            id
        }
    };
    let response = connection
        .client()
        .send_request(PromptRequest::new(session_id.clone(), vec![ContentBlock::from(text)]))
        .block_task();
    tokio::pin!(response);
    let deadline = tokio::time::sleep(Duration::from_secs(10));
    tokio::pin!(deadline);
    let mut cancelling = false;
    let result = loop {
        tokio::select! {
            result = &mut response => break result,
            command = connection.commands().recv() => match command {
                Ok(Command::Cancel) if !cancelling => {
                    cancelling = true;
                    connection.set_cancelled(true);
                    connection.client().send_notification(CancelNotification::new(session_id.clone()))?;
                    deadline.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(10));
                }
                Ok(command) => connection.handle_busy_command(command).await,
                Err(_) => return Ok(ControlFlow::Break(())),
            },
            _ = &mut deadline, if cancelling => return Err(Error::internal_error().data("Agent 未响应停止请求，连接已关闭")),
            _ = connection.client().incoming_closed() => return Err(connection::closed()),
        }
    };
    connection.emit(Event::Finished(result.map(|response| response.stop_reason))).await;
    Ok(ControlFlow::Continue(()))
}
