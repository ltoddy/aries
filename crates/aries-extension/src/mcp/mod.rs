pub mod client;
pub mod config;
pub mod error;
pub mod loader;
pub mod tool;

pub use client::McpManager;
pub use config::{McpConfig, McpServerConfig};
pub use error::{McpConnectError, McpLoadError};
pub use loader::McpConfigLoader;
pub use tool::NamespacedMcpTool;

pub type McpLoadResult<T> = Result<T, McpLoadError>;
pub type McpConnectResult<T> = Result<T, McpConnectError>;
