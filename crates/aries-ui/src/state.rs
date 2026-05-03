use std::sync::Arc;

use aries_session::SessionManager;
use tokio::sync::Mutex;

pub type SharedState = Arc<Mutex<Option<AppState>>>;
pub const CHAT_STREAM_EVENT: &str = "chat-stream";

pub struct AppState {
    pub manager: SessionManager<()>,
    pub active_session_id: String,
    pub provider: String,
    pub model: String,
}
