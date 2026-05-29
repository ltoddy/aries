use aries_core::tools::format_tool_output;
use aries_session::Session;
use rig_core::agent::MultiTurnStreamItem;
use rig_core::completion::Message;
use rig_core::message::{AssistantContent, ToolResultContent, UserContent};
use rig_core::streaming::{StreamedAssistantContent, StreamedUserContent};
use tauri::{AppHandle, Emitter};

use crate::session_service::{
    get_session, list_sessions as list_ui_sessions, load_session, putback_session,
};
use crate::state::{AppState, SharedState, CHAT_STREAM_EVENT};
use crate::types::{
    ChatBlock, ChatMessage, ChatRequest, ChatResponse, ChatStreamPayload, SessionBootstrap,
    SessionSummary,
};

/// Extract tool name from a tool-call block content (format: "[Tool]
/// <name>\n<args>").
fn extract_tool_name(tool_call_content: &str) -> &str {
    tool_call_content
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("[Tool] "))
        .unwrap_or("unknown")
}

/// Interleave tool-result blocks after their corresponding unpaired tool-call
/// blocks. Also formats each tool-result using `format_tool_output`.
fn merge_tool_results(blocks: &mut Vec<ChatBlock>, tool_result_blocks: Vec<ChatBlock>) {
    let mut result_iter = tool_result_blocks.into_iter();
    let mut i = 0;
    while i < blocks.len() {
        if blocks[i].kind == "tool-call" {
            // Check if next block is already a tool-result (already paired)
            let already_paired =
                blocks.get(i + 1).map(|b| b.kind == "tool-result").unwrap_or(false);
            if !already_paired {
                if let Some(mut tr) = result_iter.next() {
                    let tool_name = extract_tool_name(&blocks[i].content);
                    tr.content = format_tool_output(tool_name, &tr.content);
                    blocks.insert(i + 1, tr);
                    i += 2;
                    continue;
                }
            }
        }
        i += 1;
    }
    // Append any remaining tool-results that didn't find a pair
    for tr in result_iter {
        blocks.push(tr);
    }
}

fn convert_history(history: &[Message]) -> Vec<ChatMessage> {
    let mut result: Vec<ChatMessage> = Vec::new();

    for msg in history {
        match msg {
            Message::User { content } => {
                let mut text_parts: Vec<String> = Vec::new();
                let mut tool_result_blocks: Vec<ChatBlock> = Vec::new();

                for c in content.iter() {
                    match c {
                        UserContent::Text(t) => {
                            text_parts.push(t.text.clone());
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
                                tool_result_blocks.push(ChatBlock {
                                    kind: "tool-result".to_string(),
                                    content: result_text,
                                });
                            }
                        },
                        _ => {},
                    }
                }

                // If user message only contains tool results (no user text),
                // merge them into the preceding assistant message's blocks,
                // interleaving after their corresponding tool-call blocks.
                if text_parts.is_empty() && !tool_result_blocks.is_empty() {
                    if let Some(last) = result.last_mut() {
                        if last.role == "assistant" {
                            let blocks = last.blocks.get_or_insert_with(Vec::new);
                            merge_tool_results(blocks, tool_result_blocks);
                            continue;
                        }
                    }
                }

                if text_parts.is_empty() && tool_result_blocks.is_empty() {
                    continue;
                }

                result.push(ChatMessage {
                    role: "user".to_string(),
                    content: text_parts.join("\n"),
                    blocks: None,
                });
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
                                    rig_core::message::ReasoningContent::Text { text, .. } => {
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
                    continue;
                }

                // If the previous message is an assistant with tool-call/tool-result blocks,
                // this is a continuation of the same turn — merge into it.
                if let Some(last) = result.last_mut() {
                    if last.role == "assistant" {
                        if let Some(ref prev_blocks) = last.blocks {
                            if prev_blocks
                                .iter()
                                .any(|b| b.kind == "tool-call" || b.kind == "tool-result")
                            {
                                let merged_blocks = last.blocks.get_or_insert_with(Vec::new);
                                merged_blocks.extend(blocks);
                                if !text_parts.is_empty() {
                                    let sep = if last.content.is_empty() { "" } else { "\n" };
                                    last.content =
                                        format!("{}{}{}", last.content, sep, text_parts.join("\n"));
                                }
                                continue;
                            }
                        }
                    }
                }

                if blocks.iter().all(|b| b.kind == "text") {
                    result.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: text_parts.join("\n"),
                        blocks: None,
                    });
                } else {
                    result.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: text_parts.join("\n"),
                        blocks: Some(blocks),
                    });
                }
            },
            _ => {},
        }
    }

    result
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

#[tauri::command]
pub async fn list_sessions(
    state: tauri::State<'_, SharedState>,
) -> Result<Vec<SessionSummary>, String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "registry is not initialized".to_string())?;
    list_ui_sessions(app_state).await
}

#[tauri::command]
pub async fn load_session_view(
    session_id: Option<String>,
    state: tauri::State<'_, SharedState>,
) -> Result<SessionBootstrap, String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "registry is not initialized".to_string())?;

    let session = load_session(app_state, session_id).await?;
    let bootstrap = session_to_bootstrap(app_state, &session);
    putback_session(&mut app_state.registry, session);

    Ok(bootstrap)
}

#[tauri::command]
pub async fn clear_history(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "registry is not initialized".to_string())?;
    let mut session = get_session(app_state).await?;
    session.clear_history();
    putback_session(&mut app_state.registry, session);
    Ok(())
}

#[tauri::command]
pub async fn prompt(
    request: ChatRequest,
    app_handle: AppHandle,
    state: tauri::State<'_, SharedState>,
) -> Result<ChatResponse, String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "registry is not initialized".to_string())?;
    let mut session = get_session(app_state).await?;
    let session_id = session.id();

    let stream_app_handle = app_handle.clone();
    let stream_session_id = session_id.clone();
    let mut answer = String::new();
    let mut seq: u64 = 0;
    let mut last_tool_name = String::new();
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
                                    rig_core::message::ReasoningContent::Text { text, .. } => {
                                        Some(text)
                                    },
                                    _ => None,
                                })
                                .collect();
                            emit("reasoning", text)
                        },
                        StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
                            emit("reasoning", reasoning)
                        },
                        StreamedAssistantContent::ToolCall { tool_call, .. } => {
                            last_tool_name = tool_call.function.name.clone();
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
                            let raw_content: String = tool_result
                                .content
                                .iter()
                                .filter_map(|c| match c {
                                    ToolResultContent::Text(text) => Some(text.text.as_str()),
                                    _ => None,
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            if !raw_content.trim().is_empty() {
                                let formatted = format_tool_output(&last_tool_name, &raw_content);
                                emit("tool-result", formatted)
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

    putback_session(&mut app_state.registry, session);

    let content = if answer.trim().is_empty() { "Done.".to_string() } else { answer };

    Ok(ChatResponse {
        session_id,
        message: ChatMessage { role: "assistant".to_string(), content, blocks: None },
    })
}

#[tauri::command]
pub async fn get_system_prompt(state: tauri::State<'_, SharedState>) -> Result<String, String> {
    let mut guard = state.lock().await;
    let app_state = guard.as_mut().ok_or_else(|| "registry is not initialized".to_string())?;
    let session = get_session(app_state).await?;
    let system_prompt = session.system_prompt().to_string();
    putback_session(&mut app_state.registry, session);
    Ok(system_prompt)
}
