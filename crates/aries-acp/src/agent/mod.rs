use std::cell::Cell;

use agent_client_protocol::{
    AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock, ContentChunk,
    Implementation, InitializeRequest, InitializeResponse, NewSessionRequest, NewSessionResponse,
    PromptRequest, PromptResponse, ProtocolVersion, SessionNotification, SessionUpdate, StopReason,
    TextContent,
};
use aries_core::orchestrate::OrchestrateAgent;
use async_trait::async_trait;
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::info;

pub struct Agent {
    orchestrate: Mutex<OrchestrateAgent>,
    sender: mpsc::UnboundedSender<(SessionNotification, oneshot::Sender<()>)>,
    next_session_id: Cell<String>,
}

impl Agent {
    pub fn new(
        orchestrate: OrchestrateAgent,
        sender: mpsc::UnboundedSender<(SessionNotification, oneshot::Sender<()>)>,
    ) -> Self {
        let next_session_id = Cell::new(nanoid::nanoid!());

        Self { orchestrate: Mutex::new(orchestrate), sender, next_session_id }
    }
}

#[async_trait(?Send)]
impl agent_client_protocol::Agent for Agent {
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

        let session_id = self.next_session_id.take();
        self.next_session_id.set(nanoid::nanoid!());

        let resp = NewSessionResponse::new(session_id);
        Ok(resp)
    }

    async fn prompt(&self, args: PromptRequest) -> agent_client_protocol::Result<PromptResponse> {
        info!("Received prompt request {args:?}");

        let prompt_text = args
            .prompt
            .iter()
            .filter_map(|block| {
                if let ContentBlock::Text(text) = block { Some(text.text.clone()) } else { None }
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut orchestrate = self.orchestrate.lock().await;
        let stream = orchestrate.stream_prompt_v2(&prompt_text);

        tokio::pin!(stream);
        use futures::StreamExt;
        while let Some(chunk_res) = stream.next().await {
            if let Ok(chunk) = chunk_res {
                let (tx, rx) = oneshot::channel();
                if self
                    .sender
                    .send((
                        SessionNotification::new(
                            args.session_id.clone(),
                            SessionUpdate::AgentMessageChunk(ContentChunk::new(
                                ContentBlock::Text(TextContent::new(chunk)),
                            )),
                        ),
                        tx,
                    ))
                    .is_ok()
                {
                    let _ = rx.await;
                }
            }
        }

        let resp = PromptResponse::new(StopReason::EndTurn);
        Ok(resp)
    }

    async fn cancel(&self, args: CancelNotification) -> agent_client_protocol::Result<()> {
        info!("Received cancel request {args:?}");

        Ok(())
    }
}
