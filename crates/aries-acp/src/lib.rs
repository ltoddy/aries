use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};

use agent_client_protocol::{
    AgentCapabilities, AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock,
    ContentChunk, Error, ExtNotification, ExtRequest, ExtResponse, Implementation,
    InitializeRequest, InitializeResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, McpCapabilities, NewSessionRequest,
    NewSessionResponse, PromptCapabilities, PromptRequest, PromptResponse, ProtocolVersion,
    SessionCapabilities, SessionInfo, SessionListCapabilities, SessionMode, SessionModeId,
    SessionModeState, SessionNotification, SessionUpdate, SetSessionModeRequest,
    SetSessionModeResponse, StopReason, TextContent, ToolCallId, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields,
};
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_core::agents::AgentType;
use aries_core::tools::format_tool_output;
use aries_session::SessionRegistry;
use async_trait::async_trait;
use rig_core::agent::{MultiTurnStreamItem, Text};
use rig_core::message::{ReasoningContent, ToolResultContent};
use rig_core::streaming::{StreamedAssistantContent, StreamedUserContent};
use serde_json::value::RawValue;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::info;

pub struct AgentClientProtocolImpl {
    registry: Mutex<SessionRegistry>,
    sender: mpsc::UnboundedSender<(SessionNotification, oneshot::Sender<()>)>,
}

impl AgentClientProtocolImpl {
    pub async fn new(
        gctx: GlobalContext,
        config: AriesConfig,
        sender: mpsc::UnboundedSender<(SessionNotification, oneshot::Sender<()>)>,
    ) -> anyhow::Result<Self> {
        let registry = SessionRegistry::new(gctx.clone(), config).await?;

        Ok(Self { registry: Mutex::new(registry), sender })
    }
}

#[async_trait(?Send)]
impl agent_client_protocol::Agent for AgentClientProtocolImpl {
    async fn initialize(
        &self,
        args: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        info!("Received initialize request {args:?}");

        let info = Implementation::new("Aries", "0.0.1").title("Aries Agent");

        let capabilities = AgentCapabilities::new()
            .load_session(true)
            .prompt_capabilities(PromptCapabilities::new())
            .mcp_capabilities(McpCapabilities::new().http(true).sse(true))
            .session_capabilities(SessionCapabilities::new().list(SessionListCapabilities::new()));

        let resp = InitializeResponse::new(ProtocolVersion::LATEST)
            .agent_info(info)
            .agent_capabilities(capabilities);
        Ok(resp)
    }

    async fn authenticate(
        &self,
        args: AuthenticateRequest,
    ) -> agent_client_protocol::Result<AuthenticateResponse> {
        info!("Received authenticate request {args:?}");

        let resp = AuthenticateResponse::new();
        Ok(resp)
    }

    async fn new_session(
        &self,
        args: NewSessionRequest,
    ) -> agent_client_protocol::Result<NewSessionResponse> {
        info!("Received new session request {args:?}");

        let mut registry = self.registry.lock().await;

        let cwd = args.cwd.display().to_string();
        let session = registry.new_session(cwd).await?;

        let modes = Some(SessionModeState::new(
            SessionModeId::new(AgentType::Build.id()),
            vec![
                SessionMode::new(
                    SessionModeId::new(AgentType::Build.id()),
                    AgentType::Build.name(),
                )
                .description(Some(AgentType::Build.description().to_owned())),
                SessionMode::new(SessionModeId::new(AgentType::Plan.id()), AgentType::Plan.name())
                    .description(Some(AgentType::Plan.description().to_owned())),
                SessionMode::new(
                    SessionModeId::new(AgentType::General.id()),
                    AgentType::General.name(),
                )
                .description(Some(AgentType::General.description().to_owned())),
                SessionMode::new(
                    SessionModeId::new(AgentType::Explore.id()),
                    AgentType::Explore.name(),
                )
                .description(Some(AgentType::Explore.description().to_owned())),
            ],
        ));
        let resp = NewSessionResponse::new(session.id()).modes(modes);
        Ok(resp)
    }

    async fn prompt(&self, args: PromptRequest) -> agent_client_protocol::Result<PromptResponse> {
        info!("Received prompt request {args:?}");

        let session_id = args.session_id.to_string();
        let mut session = {
            let registry = self.registry.lock().await;
            registry
                .get_session(&session_id)
                .ok_or_else(|| Error::resource_not_found(Some(session_id.clone())))?
        };

        let promot = args
            .prompt
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Text(text) = block { Some(text.text.clone()) } else { None }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let tool_names = Arc::new(StdMutex::new(HashMap::new()));

        session
            .prompt(
                &promot,
                Some(|item| {
                    let sender = self.sender.clone();
                    let session_id = session_id.clone();
                    let tool_names = tool_names.clone();
                    async move {
                        let updates = stream_item_to_updates(item, &tool_names);
                        for update in updates {
                            let (tx, rx) = oneshot::channel();
                            if sender
                                .send((SessionNotification::new(session_id.clone(), update), tx))
                                .is_ok()
                            {
                                let _ = rx.await;
                            }
                        }
                        Ok(())
                    }
                }),
            )
            .await
            .map_err(|_| Error::internal_error())?;

        {
            let mut registry = self.registry.lock().await;
            registry.putback_session(session);
        }

        let resp = PromptResponse::new(StopReason::EndTurn);
        Ok(resp)
    }

    async fn cancel(&self, args: CancelNotification) -> agent_client_protocol::Result<()> {
        info!("Received cancel request {args:?}");

        let session_id = args.session_id.to_string();
        let session = {
            let registry = self.registry.lock().await;
            registry
                .get_session(&session_id)
                .ok_or_else(|| Error::resource_not_found(Some(session_id.clone())))?
        };
        session.cancel();

        Ok(())
    }

    async fn load_session(
        &self,
        args: LoadSessionRequest,
    ) -> agent_client_protocol::Result<LoadSessionResponse> {
        info!("Received load session request {args:?}");

        let session_id = args.session_id.to_string();

        let mut registry = self.registry.lock().await;
        let _ = registry.load_session(&session_id).await?;

        let modes = Some(SessionModeState::new(
            SessionModeId::new(AgentType::Build.id()),
            vec![
                SessionMode::new(
                    SessionModeId::new(AgentType::Build.id()),
                    AgentType::Build.name(),
                )
                .description(Some(AgentType::Build.description().to_owned())),
                SessionMode::new(SessionModeId::new(AgentType::Plan.id()), AgentType::Plan.name())
                    .description(Some(AgentType::Plan.description().to_owned())),
                SessionMode::new(
                    SessionModeId::new(AgentType::General.id()),
                    AgentType::General.name(),
                )
                .description(Some(AgentType::General.description().to_owned())),
                SessionMode::new(
                    SessionModeId::new(AgentType::Explore.id()),
                    AgentType::Explore.name(),
                )
                .description(Some(AgentType::Explore.description().to_owned())),
            ],
        ));
        let resp = LoadSessionResponse::new().modes(modes);
        Ok(resp)
    }

    async fn set_session_mode(
        &self,
        args: SetSessionModeRequest,
    ) -> agent_client_protocol::Result<SetSessionModeResponse> {
        info!("Received set session mode request {args:?}");

        let session_id = args.session_id.to_string();
        let mut session = {
            let registry = self.registry.lock().await;
            registry
                .get_session(&session_id)
                .ok_or_else(|| Error::resource_not_found(Some(session_id.clone())))?
        };

        let mode_id = args.mode_id.to_string();
        let agent_type = AgentType::from_id(&mode_id);

        session.switch_agent(agent_type).await?;

        let resp = SetSessionModeResponse::new();
        Ok(resp)
    }

    async fn list_sessions(
        &self,
        args: ListSessionsRequest,
    ) -> agent_client_protocol::Result<ListSessionsResponse> {
        info!("Received list sessions request {args:?}");

        let mut registry = self.registry.lock().await;

        let sessions = registry.list_sessions(args.cwd).await?;
        let sessions = sessions
            .into_iter()
            .map(|s| {
                SessionInfo::new(s.session_id, s.cwd)
                    .title(s.title)
                    .updated_at(s.updated_at.to_string())
            })
            .collect();

        let resp = ListSessionsResponse::new(sessions);
        Ok(resp)
    }

    async fn ext_method(&self, args: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        info!("Received ext method request {args:?}");

        let resp = ExtResponse::new(RawValue::NULL.to_owned().into());
        Ok(resp)
    }

    async fn ext_notification(&self, args: ExtNotification) -> agent_client_protocol::Result<()> {
        info!("Received ext notification request {args:?}");

        Ok(())
    }
}

fn stream_item_to_updates(
    item: MultiTurnStreamItem<()>,
    tool_names: &Arc<StdMutex<HashMap<String, String>>>,
) -> Vec<SessionUpdate> {
    match item {
        MultiTurnStreamItem::StreamAssistantItem(assistant) => {
            assistant_to_updates(assistant, tool_names)
        },
        MultiTurnStreamItem::StreamUserItem(user) => user_to_updates(user, tool_names),
        MultiTurnStreamItem::FinalResponse(_) => Vec::new(),
        _ => Vec::new(),
    }
}

fn assistant_to_updates(
    content: StreamedAssistantContent<()>,
    tool_names: &Arc<StdMutex<HashMap<String, String>>>,
) -> Vec<SessionUpdate> {
    match content {
        StreamedAssistantContent::Text(Text { text }) => {
            vec![SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(text),
            )))]
        },
        StreamedAssistantContent::Reasoning(reasoning) => reasoning
            .content
            .into_iter()
            .filter_map(|rc| match rc {
                ReasoningContent::Text { text, .. } => Some(text),
                ReasoningContent::Encrypted(s) => Some(s),
                ReasoningContent::Redacted { data } => Some(data),
                ReasoningContent::Summary(s) => Some(s),
                _ => None,
            })
            .map(|text| {
                SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(
                    TextContent::new(text),
                )))
            })
            .collect(),
        StreamedAssistantContent::ReasoningDelta { reasoning, .. } => {
            vec![SessionUpdate::AgentThoughtChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new(reasoning),
            )))]
        },
        StreamedAssistantContent::ToolCall { tool_call, internal_call_id, .. } => {
            if let Ok(mut tool_names) = tool_names.lock() {
                tool_names.insert(internal_call_id, tool_call.function.name.clone());
            }

            let arguments = tool_call.function.arguments.to_string();
            let tool_call = agent_client_protocol::ToolCall::new(
                ToolCallId::new(&*tool_call.id),
                &tool_call.function.name,
            )
            .status(ToolCallStatus::InProgress)
            .raw_input(serde_json::Value::String(arguments));
            vec![SessionUpdate::ToolCall(tool_call)]
        },
        _ => Vec::new(),
    }
}

fn user_to_updates(
    content: StreamedUserContent,
    tool_names: &Arc<StdMutex<HashMap<String, String>>>,
) -> Vec<SessionUpdate> {
    match content {
        StreamedUserContent::ToolResult { tool_result, internal_call_id } => {
            let raw_content = tool_result
                .content
                .iter()
                .filter_map(|c| match c {
                    ToolResultContent::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            let tool_name = match tool_names.lock() {
                Ok(mut tool_names) => tool_names.remove(&internal_call_id),
                Err(_) => None,
            };
            let formatted = match tool_name {
                Some(name) => format_tool_output(&name, &raw_content),
                None => raw_content,
            };

            let fields = ToolCallUpdateFields::new()
                .status(ToolCallStatus::Completed)
                .raw_output(serde_json::Value::String(formatted));
            vec![SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                ToolCallId::new(&*tool_result.id),
                fields,
            ))]
        },
    }
}
