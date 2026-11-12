use std::path::PathBuf;
use std::time::Duration;

use agent_client_protocol::v1::schema::{NewSessionRequest, NewSessionResponse};

use crate::acp::Command;
use crate::acp::connection::{self, AgentConnection};
use crate::acp::requests::RequestError;

pub async fn request(
    connection: &AgentConnection,
    cwd: impl Into<PathBuf>,
) -> Result<Option<NewSessionResponse>, RequestError> {
    let cwd = cwd.into();
    if !cwd.is_absolute() {
        return Err(RequestError::InvalidCwd);
    }
    let create = tokio::time::timeout(
        Duration::from_secs(30),
        connection.client().send_request(NewSessionRequest::new(cwd)).block_task(),
    );
    tokio::pin!(create);
    loop {
        tokio::select! {
            biased;
            command = connection.commands().recv() => match command {
                Ok(Command::Cancel) => return Err(RequestError::CreationCancelled),
                Ok(command) => connection.handle_busy_command(command).await,
                Err(_) => return Ok(None),
            },
            _ = connection.client().incoming_closed() => return Err(connection::closed().into()),
            result = &mut create => {
                let response = result.map_err(|source| RequestError::Timeout { message: "创建会话超时", source })??;
                return Ok(Some(response));
            }
        }
    }
}
