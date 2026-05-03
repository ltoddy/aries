use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use rig::agent::MultiTurnStreamItem;
use rig::message::ToolResultContent;
use rig::streaming::{StreamedAssistantContent, StreamedUserContent};
use tauri::{AppHandle, Emitter};

use crate::state::{AppState, SharedState, CHAT_STREAM_EVENT};
use crate::types::{ChatMessage, ChatRequest, ChatResponse, ChatStreamPayload, SessionBootstrap};

#[tauri::command]
pub async fn bootstrap_chat(
    state: tauri::State<'_, SharedState>,
) -> Result<SessionBootstrap, String> {
    let mut guard = state.lock().await;

    if guard.is_none() {
        let gctx = GlobalContext::new().map_err(|err| err.to_string())?;
        let loader = AriesConfigLoader::new(&gctx.config_dir);
        let config = loader.load_or_setup().await.map_err(|err| err.to_string())?;
        let provider = config.provider().to_string();
        let model = config.model().to_string();

        let mut manager = aries_session::SessionManager::new(gctx, config, ());
        let session_id = manager.create_session().await.map_err(|err| err.to_string())?;

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
            messages: vec![],
        });
    }

    let app_state = guard.as_ref().expect("state initialized");

    Ok(SessionBootstrap {
        app_name: "Aries",
        provider: app_state.provider.clone(),
        model: app_state.model.clone(),
        session_id: app_state.active_session_id.clone(),
        messages: vec![],
    })
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

    session
        .prompt(
            request.prompt.trim(),
            Some(|event: MultiTurnStreamItem<()>| {
                let app_handle = stream_app_handle.clone();
                let session_id = stream_session_id.clone();

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
