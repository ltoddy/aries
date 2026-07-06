pub mod client;
pub mod config;
pub mod error;
pub mod loader;
pub mod tool;

pub use client::McpManager;
pub use config::{HttpConfig, McpConfig, McpServerConfig, SseConfig, StdioConfig};
pub use error::{McpConnectError, McpParseError};
pub use loader::McpConfigLoader;
pub use tool::NamespacedMcpTool;

pub type McpLoadResult<T> = Result<T, McpParseError>;
pub type McpConnectResult<T> = Result<T, McpConnectError>;
