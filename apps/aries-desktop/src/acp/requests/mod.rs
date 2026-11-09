use agent_client_protocol::Error;
use tokio::time::error::Elapsed;

#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    #[error(transparent)]
    Protocol(#[from] Error),
    #[error("会话工作目录必须是绝对路径")]
    InvalidCwd,
    #[error("{message}")]
    Timeout {
        message: &'static str,
        #[source]
        source: Elapsed,
    },
    #[error("已停止创建会话")]
    CreationCancelled,
    #[error("Agent 未协商 ACP v1")]
    UnsupportedVersion,
    #[error("Agent 返回了重复的会话分页游标")]
    RepeatedCursor,
    #[error("Agent 未响应停止请求，连接已关闭")]
    CancellationTimeout,
}

impl From<RequestError> for Error {
    fn from(error: RequestError) -> Self {
        match error {
            RequestError::Protocol(error) => error,
            RequestError::InvalidCwd => Self::invalid_params().data(error.to_string()),
            RequestError::Timeout { .. }
            | RequestError::CreationCancelled
            | RequestError::UnsupportedVersion
            | RequestError::RepeatedCursor
            | RequestError::CancellationTimeout => Self::internal_error().data(error.to_string()),
        }
    }
}

pub mod delete_sessions;
pub mod initialize;
pub mod list_sessions;
pub mod load_session;
pub mod new_session;
pub mod prompt;
pub mod set_session_config_option;
