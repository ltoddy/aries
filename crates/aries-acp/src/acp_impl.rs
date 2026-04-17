use std::cell::Cell;

use agent_client_protocol::{
    AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock, ContentChunk,
    Error, Implementation, InitializeRequest, InitializeResponse, NewSessionRequest,
    NewSessionResponse, PromptRequest, PromptResponse, ProtocolVersion, SessionNotification,
    SessionUpdate, StopReason, TextContent, ToolCallId, ToolCallStatus, ToolCallUpdate,
    ToolCallUpdateFields,
};
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_session::{SessionManager, StreamEvent};
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::info;

pub struct AcpImpl {
    sessions: Mutex<SessionManager>,
    sender: mpsc::UnboundedSender<(SessionNotification, oneshot::Sender<()>)>,
    next_session_id: Cell<String>,
}

impl AcpImpl {
    pub fn new(
        context: GlobalContext,
        config: AriesConfig,
        sender: mpsc::UnboundedSender<(SessionNotification, oneshot::Sender<()>)>,
    ) -> Self {
        let next_session_id = Cell::new(nanoid::nanoid!());
        let sessions = SessionManager::new(context, config, ());

        Self { sessions: Mutex::new(sessions), sender, next_session_id }
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

        let mut sessions = self.sessions.lock().await;
        let session_id = sessions.create_session().map_err(|_| Error::internal_error())?;
        self.next_session_id.set(nanoid::nanoid!());

        let resp = NewSessionResponse::new(session_id);
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

        let mut sessions = self.sessions.lock().await;
        let session_id = args.session_id.to_string();
        let session = sessions.get_session_mut(&session_id).ok_or_else(Error::internal_error)?;
        session
            .prompt(
                &promot,
                Some(|event| {
                    let sender = self.sender.clone();
                    let session_id = args.session_id.clone();
                    async move {
                        let update = match event {
                            StreamEvent::Text(text) => SessionUpdate::AgentMessageChunk(
                                ContentChunk::new(ContentBlock::Text(TextContent::new(text))),
                            ),
                            StreamEvent::Reasoning(text) => SessionUpdate::AgentThoughtChunk(
                                ContentChunk::new(ContentBlock::Text(TextContent::new(text))),
                            ),
                            StreamEvent::ToolCall { id, name, arguments } => {
                                let tool_call = agent_client_protocol::ToolCall::new(
                                    ToolCallId::new(&*id),
                                    &name,
                                )
                                .status(ToolCallStatus::InProgress)
                                .raw_input(serde_json::Value::String(arguments));
                                SessionUpdate::ToolCall(tool_call)
                            },
                            StreamEvent::ToolResult { id, content } => {
                                let fields = ToolCallUpdateFields::new()
                                    .status(ToolCallStatus::Completed)
                                    .raw_output(serde_json::Value::String(content));
                                SessionUpdate::ToolCallUpdate(ToolCallUpdate::new(
                                    ToolCallId::new(&*id),
                                    fields,
                                ))
                            },
                        };
                        let (tx, rx) = oneshot::channel();
                        if sender.send((SessionNotification::new(session_id, update), tx)).is_ok() {
                            let _ = rx.await;
                        }

                        Ok(())
                    }
                }),
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
