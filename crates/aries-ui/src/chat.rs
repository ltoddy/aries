use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use rig::agent::MultiTurnStreamItem;
use rig::completion::Message;
use rig::message::{AssistantContent, ToolResultContent, UserContent};
use rig::streaming::{StreamedAssistantContent, StreamedUserContent};
use tauri::{AppHandle, Emitter};

use crate::state::{AppState, SharedState, CHAT_STREAM_EVENT};
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ChatStreamPayload, SessionBootstrap};

fn convert_history(history: &[Message]) -> Vec<ChatMessage> {
    history
        .iter()
        .filter_map(|msg| match msg {
            Message::User { content } => {
                let text: String = content
                    .iter()
                    .filter_map(|c| match c {
                        UserContent::Text(t) => Some(t.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.is_empty() {
                    None
                } else {
                    Some(ChatMessage { role: "user".to_string(), content: text })
                }
            },
            Message::Assistant { content, .. } => {
                let text: String = content
                    .iter()
                    .filter_map(|c| match c {
                        AssistantContent::Text(t) => Some(t.text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                if text.is_empty() {
                    None
                } else {
                    Some(ChatMessage { role: "assistant".to_string(), content: text })
                }
            },
            _ => None,
        })
        .collect()
}

#[tauri::command]
pub async fn bootstrap_chat(
    project_path: String,
    state: tauri::State<'_, SharedState>,
) -> Result<SessionBootstrap, String> {
    let mut guard = state.lock().await;

    if guard.is_none() {
        let mut gctx = GlobalContext::new().map_err(|err| err.to_string())?;
        gctx.current_dir = std::path::PathBuf::from(&project_path);
        let loader = AriesConfigLoader::new(&gctx.config_dir);
        let config = loader.load_or_setup().await.map_err(|err| err.to_string())?;
        let provider = config.provider().to_string();
        let model = config.model().to_string();

        let mut manager = aries_session::SessionManager::new(gctx, config, ());
        let session_id = manager.create_session().await.map_err(|err| err.to_string())?;

        let (messages, session_dir_name) = manager
            .get_session(&session_id)
            .map(|s| {
                (
                    convert_history(s.history()),
                    s.dir()
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default();

        *guard = Some(AppState {
            manager,
            active_session_id: session_id.clone(),
            provider: provider.clone(),
            model: model.clone(),
        });

        return Ok(SessionBootstrap {
            app_name: "Aries",
            provider,
            model,
            session_id,
            session_dir_name,
            messages,
        });
    }

    let app_state = guard.as_ref().expect("state initialized");
    let (messages, session_dir_name) = app_state
        .manager
        .get_session(&app_state.active_session_id)
        .map(|s| {
            (
                convert_history(s.history()),
                s.dir().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            )
        })
        .unwrap_or_default();

    Ok(SessionBootstrap {
        app_name: "Aries",
        provider: app_state.provider.clone(),
        model: app_state.model.clone(),
        session_id: app_state.active_session_id.clone(),
        session_dir_name,
        messages,
    })
}

#[tauri::command]
pub async fn clear_history(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "chat session is not initialized".to_string())?;
    let session_id = app_state.active_session_id.clone();
    let session = app_state
        .manager
        .get_session_mut(&session_id)
        .ok_or_else(|| "active session not found".to_string())?;
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
    let app_state = guard.as_mut().ok_or_else(|| "chat session is not initialized".to_string())?;
    let session_id = app_state.active_session_id.clone();
    let session = app_state
        .manager
        .get_session_mut(&session_id)
        .ok_or_else(|| "active session not found".to_string())?;

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
        )
        .await
        .map_err(|err| err.to_string())?;

    let content = if answer.trim().is_empty() { "Done.".to_string() } else { answer };

    Ok(ChatResponse { session_id, message: ChatMessage { role: "assistant".to_string(), content } })
}

#[tauri::command]
pub async fn get_system_prompt(state: tauri::State<'_, SharedState>) -> Result<String, String> {
    let guard = state.lock().await;
    let app_state = guard.as_ref().ok_or_else(|| "chat session is not initialized".to_string())?;
    let session_id = &app_state.active_session_id;
    let session = app_state
        .manager
        .get_session(session_id)
        .ok_or_else(|| "active session not found".to_string())?;
    Ok(session.system_prompt().to_string())
}
