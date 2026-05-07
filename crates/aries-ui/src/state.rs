use std::sync::Arc;

use aries_session::{Session, SessionRegistry};
use tokio::sync::Mutex;

pub type SharedState = Arc<Mutex<Option<AppState>>>;
pub const CHAT_STREAM_EVENT: &str = "chat-stream";

pub struct AppState {
    pub registry: SessionRegistry,
    pub provider: String,
    pub model: String,
    pub active_project_dir: Option<String>,
    pub active_session: Option<Session>,
}
