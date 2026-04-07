use std::pin::Pin;
use std::task::{Context, Poll};

use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_core::AgentWrapper;
use aries_core::agent_type::AgentType;
use aries_core::compaction::CompactionAgent;
use futures::Stream;
use rig::agent::{FinalResponse, MultiTurnStreamItem, StreamingError, StreamingResult};
use rig::completion::{self, Message};
use rig::providers::openai;

type SessionStreamingResponse =
    <openai::CompletionModel as completion::CompletionModel>::StreamingResponse;

pub struct SessionPromptStream<'a> {
    session: &'a mut Session,
    stream: StreamingResult<SessionStreamingResponse>,
}

impl Unpin for SessionPromptStream<'_> {}

impl Stream for SessionPromptStream<'_> {
    type Item = Result<MultiTurnStreamItem<SessionStreamingResponse>, StreamingError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.as_mut().get_mut();
        match this.stream.as_mut().poll_next(cx) {
            Poll::Ready(Some(Ok(MultiTurnStreamItem::FinalResponse(response)))) => {
                this.session.update_history_from_response(&response);
                Poll::Ready(Some(Ok(MultiTurnStreamItem::FinalResponse(response))))
            },
            other => other,
        }
    }
}

pub struct Session {
    id: String,
    agent: AgentWrapper,
    compaction_agent: CompactionAgent,
    history: Vec<Message>,
    base_history_len: usize,
}

impl Session {
    pub fn new(id: String, context: &GlobalContext, config: AriesConfig) -> anyhow::Result<Self> {
        let history = vec![Message::user(format!("当前目录：{}", context.current_dir.display()))];
        let base_history_len = history.len();
        let agent = AgentWrapper::new(
            format!("Session Agent {}", id),
            config.clone(),
            AgentType::Orchestrate,
        )?;
        let compaction_agent = CompactionAgent::new(config.clone())?;

        Ok(Self { id, agent, compaction_agent, history, base_history_len })
    }

    pub async fn stream_prompt(&mut self, prompt: &str) -> SessionPromptStream<'_> {
        let _ = self.compact_if_needed().await;
        let history = self.history.clone();
        let stream = Box::pin(self.agent.stream_prompt(prompt, &history).await);
        SessionPromptStream { session: self, stream }
    }

    pub async fn compact_if_needed(&mut self) -> anyhow::Result<Option<String>> {
        let Some(summary) = self.compaction_agent.compact(self.history.clone()).await? else {
            return Ok(None);
        };

        // 清理 session 的历史并添加摘要
        self.history.truncate(self.base_history_len);
        self.history.push(Message::assistant(summary.clone()));
        // 不需要同步到 agent，因为 agent 不再维护自己的历史

        Ok(Some(summary))
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub fn clear_history(&mut self) {
        self.history.truncate(self.base_history_len);
    }

    fn update_history_from_response(&mut self, response: &FinalResponse) {
        if let Some(history) = response.history() {
            self.history = history.to_vec();
        }
    }
}
