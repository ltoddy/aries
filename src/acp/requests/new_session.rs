use std::path::PathBuf;
use std::time::Duration;

use agent_client_protocol::Error;
use agent_client_protocol::v1::schema::{NewSessionRequest, NewSessionResponse};

use crate::acp::Command;
use crate::acp::connection::{self, AgentConnection};

pub async fn request(
    connection: &AgentConnection,
    cwd: PathBuf,
) -> Result<Option<NewSessionResponse>, Error> {
    if !cwd.is_absolute() {
        return Err(Error::invalid_params().data("会话工作目录必须是绝对路径"));
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
                Ok(Command::Cancel) => return Err(Error::internal_error().data("已停止创建会话")),
                Ok(command) => connection.handle_busy_command(command).await,
                Err(_) => return Ok(None),
            },
            _ = connection.client().incoming_closed() => return Err(connection::closed()),
            result = &mut create => {
                let response = result.map_err(|_| Error::internal_error().data("创建会话超时"))??;
                return Ok(Some(response));
            }
        }
    }
}
