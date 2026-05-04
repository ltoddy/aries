use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionBootstrap {
    pub app_name: &'static str,
    pub provider: String,
    pub model: String,
    pub session_id: String,
    pub session_dir_name: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub prompt: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub session_id: String,
    pub message: ChatMessage,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChatStreamPayload {
    pub seq: u64,
    pub session_id: String,
    pub kind: String,
    pub delta: String,
}
