use std::cell::Cell;
use std::path::PathBuf;

use agent_client_protocol::{
    AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock, ContentChunk,
    Error, Implementation, InitializeRequest, InitializeResponse, NewSessionRequest,
    NewSessionResponse, PromptRequest, PromptResponse, ProtocolVersion, SessionNotification,
    SessionUpdate, StopReason, TextContent, ToolCallId, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields,
};
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_session::SessionRegistry;
use async_trait::async_trait;
use rig::agent::{MultiTurnStreamItem, Text};
use rig::message::{ReasoningContent, ToolResultContent};
use rig::streaming::{StreamedAssistantContent, StreamedUserContent};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::info;

pub struct AcpImpl {
    current_dir: PathBuf,
    registry: Mutex<SessionRegistry>,
    sender: mpsc::UnboundedSender<(SessionNotification, oneshot::Sender<()>)>,
    next_session_id: Cell<String>,
}

impl AcpImpl {
    pub async fn new(
        gctx: GlobalContext,
        config: AriesConfig,
        sender: mpsc::UnboundedSender<(SessionNotification, oneshot::Sender<()>)>,
    ) -> anyhow::Result<Self> {
        let next_session_id = Cell::new(nanoid::nanoid!());
        let registry = SessionRegistry::new(gctx.clone(), config).await?;

        Ok(Self {
            current_dir: gctx.current_dir,
            registry: Mutex::new(registry),
            sender,
            next_session_id,
        })
    }
}

#[async_trait(?Send)]
impl agent_client_protocol::Agent for AcpImpl {
    async fn initialize(
        &self,
        args: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        info!("Received initialize request {args:?}");

        let info = Implementation::new("aries", "0.1.0").title("Aries Agent");
        let resp = InitializeResponse::new(ProtocolVersion::LATEST).agent_info(info);
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

        let current_dir = self.current_dir.display().to_string();
        let mut registry = self.registry.lock().await;
        let session_id = nanoid::nanoid!();
        let session = registry
            .get_session(&current_dir, &session_id)
            .await
            .map_err(|_| Error::internal_error())?;
        self.next_session_id.set(session.id());

        let resp = NewSessionResponse::new(session.id());
        Ok(resp)
    }

    async fn prompt(&self, args: PromptRequest) -> agent_client_protocol::Result<PromptResponse> {
        info!("Received prompt request {args:?}");

        let promot = args
            .prompt
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Text(text) = block { Some(text.text.clone()) } else { None }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut registry = self.registry.lock().await;
        let session_id = args.session_id.to_string();
        let current_dir = self.current_dir.display().to_string();

        let mut session = registry.get_session(&current_dir, &session_id).await?;
        session
            .prompt(
                &promot,
                Some(|item| {
                    let sender = self.sender.clone();
                    let session_id = args.session_id.clone();
                    async move {
                        let updates = stream_item_to_updates(item);
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
                (),
            )
            .await
            .map_err(|_| Error::internal_error())?;

        let resp = PromptResponse::new(StopReason::EndTurn);
        Ok(resp)
    }

    async fn cancel(&self, args: CancelNotification) -> agent_client_protocol::Result<()> {
        info!("Received cancel request {args:?}");

        Ok(())
    }
}

fn stream_item_to_updates(item: MultiTurnStreamItem<()>) -> Vec<SessionUpdate> {
    match item {
        MultiTurnStreamItem::StreamAssistantItem(assistant) => assistant_to_updates(assistant),
        MultiTurnStreamItem::StreamUserItem(user) => user_to_updates(user),
        MultiTurnStreamItem::FinalResponse(_) => Vec::new(),
        _ => Vec::new(),
    }
}

fn assistant_to_updates(content: StreamedAssistantContent<()>) -> Vec<SessionUpdate> {
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
        StreamedAssistantContent::ToolCall { tool_call, .. } => {
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

fn user_to_updates(content: StreamedUserContent) -> Vec<SessionUpdate> {
    match content {
        StreamedUserContent::ToolResult { tool_result, .. } => {
            let content = tool_result
                .content
                .iter()
                .filter_map(|c| match c {
                    ToolResultContent::Text(text) => Some(text.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            let fields = ToolCallUpdateFields::new()
                .status(ToolCallStatus::Completed)
                .raw_output(serde_json::Value::String(content));
            vec![SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                ToolCallId::new(&*tool_result.id),
                fields,
            ))]
        },
    }
}
