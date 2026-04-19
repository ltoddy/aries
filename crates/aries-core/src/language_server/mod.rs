mod client;
mod detection;

pub use client::LspClient;
pub use detection::{LspServerInfo, detect_language_server, is_binary_installed};
