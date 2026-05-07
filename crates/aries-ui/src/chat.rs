use aries_session::Session;
use rig::agent::MultiTurnStreamItem;
use rig::completion::Message;
use rig::message::{AssistantContent, ToolResultContent, UserContent};
use rig::streaming::{StreamedAssistantContent, StreamedUserContent};
use tauri::{AppHandle, Emitter};

use crate::state::{AppState, SharedState, CHAT_STREAM_EVENT};
use crate::types::{
    ChatBlock, ChatMessage, ChatRequest, ChatResponse, ChatStreamPayload, SessionBootstrap,
    SessionSummary,
};

fn convert_history(history: &[Message]) -> Vec<ChatMessage> {
    history
        .iter()
        .filter_map(|msg| match msg {
            Message::User { content } => {
                let mut text_parts: Vec<String> = Vec::new();
                let mut blocks: Vec<ChatBlock> = Vec::new();
                for c in content.iter() {
                    match c {
                        UserContent::Text(t) => {
                            text_parts.push(t.text.clone());
                            blocks.push(ChatBlock {
                                kind: "text".to_string(),
                                content: t.text.clone(),
                            });
                        },
                        UserContent::ToolResult(tr) => {
                            let result_text: String = tr
                                .content
                                .iter()
                                .filter_map(|c| match c {
                                    ToolResultContent::Text(text) => Some(text.text.as_str()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            if !result_text.trim().is_empty() {
                                blocks.push(ChatBlock {
                                    kind: "tool-result".to_string(),
                                    content: result_text,
                                });
                            }
                        },
                        _ => {},
                    }
                }
                if text_parts.is_empty() && blocks.is_empty() {
                    None
                } else if blocks.iter().all(|b| b.kind == "text") {
                    Some(ChatMessage {
                        role: "user".to_string(),
                        content: text_parts.join("\n"),
                        blocks: None,
                    })
                } else {
                    Some(ChatMessage {
                        role: "user".to_string(),
                        content: text_parts.join("\n"),
                        blocks: Some(blocks),
                    })
                }
            },
            Message::Assistant { content, .. } => {
                let mut text_parts: Vec<String> = Vec::new();
                let mut blocks: Vec<ChatBlock> = Vec::new();
                for c in content.iter() {
                    match c {
                        AssistantContent::Text(t) => {
                            text_parts.push(t.text.clone());
                            blocks.push(ChatBlock {
                                kind: "text".to_string(),
                                content: t.text.clone(),
                            });
                        },
                        AssistantContent::Reasoning(r) => {
                            let text: String = r
                                .content
                                .iter()
                                .filter_map(|rc| match rc {
                                    rig::message::ReasoningContent::Text { text, .. } => {
                                        Some(text.clone())
                                    },
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            if !text.trim().is_empty() {
                                blocks.push(ChatBlock {
                                    kind: "reasoning".to_string(),
                                    content: text,
                                });
                            }
                        },
                        AssistantContent::ToolCall(tc) => {
                            let delta =
                                format!("[Tool] {}\n{}", tc.function.name, tc.function.arguments);
                            blocks
                                .push(ChatBlock { kind: "tool-call".to_string(), content: delta });
                        },
                        _ => {},
                    }
                }
                if text_parts.is_empty() && blocks.is_empty() {
                    None
                } else if blocks.iter().all(|b| b.kind == "text") {
                    Some(ChatMessage {
                        role: "assistant".to_string(),
                        content: text_parts.join("\n"),
                        blocks: None,
                    })
                } else {
                    Some(ChatMessage {
                        role: "assistant".to_string(),
                        content: text_parts.join("\n"),
                        blocks: Some(blocks),
                    })
                }
            },
            _ => None,
        })
        .collect()
}

fn session_to_bootstrap(app_state: &AppState, session: &Session) -> SessionBootstrap {
    let session_dir_name =
        session.dir().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    SessionBootstrap {
        app_name: "Aries",
        provider: app_state.provider.clone(),
        model: app_state.model.clone(),
        session_id: session.id(),
        session_dir_name,
        messages: convert_history(session.history()),
    }
}

fn require_project_dir(app_state: &AppState) -> Result<String, String> {
    app_state.active_project_dir.clone().ok_or_else(|| "no active project".to_string())
}

#[tauri::command]
pub async fn list_sessions(
    state: tauri::State<'_, SharedState>,
) -> Result<Vec<SessionSummary>, String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "registry is not initialized".to_string())?;
    let project_dir = require_project_dir(app_state)?;

    let sessions = app_state
        .registry
        .list_sessions(&project_dir)
        .await
        .map_err(|err| err.to_string())?;

    Ok(sessions
        .into_iter()
        .map(|s| SessionSummary {
            id: s.session_id.clone(),
            session_id: s.session_id,
            title: s.title,
            project_dir: s.project_dir,
        })
        .collect())
}

#[tauri::command]
pub async fn bootstrap_chat(
    session_id: Option<String>,
    state: tauri::State<'_, SharedState>,
) -> Result<SessionBootstrap, String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "registry is not initialized".to_string())?;
    let project_dir = require_project_dir(app_state)?;

    let sid = session_id.unwrap_or_else(|| nanoid::nanoid!());
    let session = app_state
        .registry
        .get_session(&project_dir, &sid)
        .await
        .map_err(|err| err.to_string())?;

    let bootstrap = session_to_bootstrap(app_state, &session);
    app_state.active_session = Some(session);
    Ok(bootstrap)
}

#[tauri::command]
pub async fn clear_history(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "registry is not initialized".to_string())?;
    let session =
        app_state.active_session.as_mut().ok_or_else(|| "active session not found".to_string())?;
    session.clear_history();
    Ok(())
}

#[tauri::command]
pub async fn send_chat_message(
    request: ChatRequest,
    app_handle: AppHandle,
    state: tauri::State<'_, SharedState>,
) -> Result<ChatResponse, String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "registry is not initialized".to_string())?;
    let session =
        app_state.active_session.as_mut().ok_or_else(|| "active session not found".to_string())?;
    let session_id = session.id();

    let stream_app_handle = app_handle.clone();
    let stream_session_id = session_id.clone();
    let mut answer = String::new();
    let mut seq: u64 = 0;
    let start_time = std::time::Instant::now();

    session
        .prompt(
            request.prompt.trim(),
            Some(|event: MultiTurnStreamItem<()>| {
                let app_handle = stream_app_handle.clone();
                let session_id = stream_session_id.clone();
                let elapsed_ms = start_time.elapsed().as_millis();

                seq += 1;
                let current_seq = seq;

                let emit = move |kind: &str, delta: String| -> anyhow::Result<()> {
                    if delta.is_empty() {
                        return Ok(());
                    }

                    app_handle.emit(
                        CHAT_STREAM_EVENT,
                        ChatStreamPayload {
                            seq: current_seq,
                            session_id: session_id.clone(),
                            kind: kind.to_string(),
                            delta,
                        },
                    )?;
                    Ok(())
                };

                let result = match event {
                    MultiTurnStreamItem::StreamAssistantItem(assistant) => match assistant {
                        StreamedAssistantContent::Text(t) => {
                            answer.push_str(&t.text);
                            emit("text", t.text)
                        },
                        StreamedAssistantContent::Reasoning(r) => {
                            let text: String = r
                                .content
                                .into_iter()
                                .filter_map(|rc| match rc {
                                    rig::message::ReasoningContent::Text { text, .. } => Some(text),
                                    _ => None,
                                })
                                .collect();
                            emit("reasoning", text)
                        },
                        StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                            emit("reasoning", reasoning)
                        },
                        StreamedAssistantContent::ToolCall { tool_call, .. } => {
                            let delta = format!(
                                "[Tool] {}\n{}",
                                tool_call.function.name, tool_call.function.arguments
                            );
                            emit("tool-call", delta)
                        },
                        _ => Ok(()),
                    },
                    MultiTurnStreamItem::StreamUserItem(user) => match user {
                        StreamedUserContent::ToolResult { tool_result, .. } => {
                            let content: String = tool_result
                                .content
                                .iter()
                                .filter_map(|c| match c {
                                    ToolResultContent::Text(text) => Some(text.text.as_str()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            if !content.trim().is_empty() {
                                emit("tool-result", content)
                            } else {
                                Ok(())
                            }
                        },
                    },
                    MultiTurnStreamItem::FinalResponse(ref response) => {
                        let usage = response.usage();
                        let delta = serde_json::json!({
                            "total_tokens": usage.total_tokens,
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                            "cached_input_tokens": usage.cached_input_tokens,
                            "elapsed_ms": elapsed_ms,
                        })
                        .to_string();
                        emit("usage", delta)
                    },
                    _ => Ok(()),
                };

                async move { result }
            }),
            (),
        )
        .await
        .map_err(|err| err.to_string())?;

    let content = if answer.trim().is_empty() { "Done.".to_string() } else { answer };

    Ok(ChatResponse {
        session_id,
        message: ChatMessage { role: "assistant".to_string(), content, blocks: None },
    })
}

#[tauri::command]
pub async fn get_system_prompt(state: tauri::State<'_, SharedState>) -> Result<String, String> {
    let guard = state.lock().await;
    let app_state = guard.as_ref().ok_or_else(|| "registry is not initialized".to_string())?;
    let session =
        app_state.active_session.as_ref().ok_or_else(|| "active session not found".to_string())?;
    Ok(session.system_prompt().to_string())
}
