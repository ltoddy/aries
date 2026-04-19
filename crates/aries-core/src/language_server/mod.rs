mod client;
mod detection;

use std::sync::Arc;

pub use client::LspClient;
pub use detection::{LspServerInfo, detect_language_server, is_binary_installed};
use tokio::sync::Mutex;

pub type SharedLspClient = Arc<Mutex<Option<LspClient>>>;
