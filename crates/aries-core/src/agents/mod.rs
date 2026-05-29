pub mod agent_type;
pub mod builder;
pub mod compaction;
pub mod summary;
pub mod title;

use futures::StreamExt;
use rig_core::agent::{Agent, FinalResponse, MultiTurnStreamItem, StreamingResult};
use rig_core::completion::{CompletionModel, Message};
use rig_core::streaming::StreamingPrompt;
use rig_core::wasm_compat::WasmCompatSend;

pub use self::agent_type::AgentType;
pub use self::builder::AgentBuilder;
pub use self::compaction::CompactionAgent;
pub use self::summary::SummaryAgent;
pub use self::title::TitleAgent;
use crate::AriesResult;
use crate::error::AgentError;

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

#[derive(Clone)]
pub struct AriesAgent<M>
where
    M: CompletionModel,
{
    inner: Agent<M>,

    preamble: String,
    name: String,
}

impl<M> AriesAgent<M>
where
    M: CompletionModel,
{
    pub fn new(inner: Agent<M>, name: impl Into<String>, preamble: impl Into<String>) -> Self {
        let name = name.into();
        let preamble = preamble.into();

        Self { inner, preamble, name }
    }
}

impl<M> AriesAgent<M>
where
    M: CompletionModel + 'static,
{
    pub async fn stream_prompt(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: &[Message],
    ) -> StreamingResult<<M>::StreamingResponse> {
        self.inner.stream_prompt(prompt).with_history(history).await
    }

    pub async fn completion(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: &[Message],
    ) -> AriesResult<String, AgentError> {
        let stream = self.inner.stream_prompt(prompt).with_history(history).await;
        futures::pin_mut!(stream);

        let mut final_res = FinalResponse::empty();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(item) => {
                    if let MultiTurnStreamItem::FinalResponse(res) = item {
                        final_res = res;
                    }
                },
                Err(e) => {
                    return Err(AgentError::ExecutionError(format!("Mainagent failed: {}", e)));
                },
            }
        }

        Ok(final_res.response().to_owned())
    }

    pub fn system_prompt(&self) -> &str {
        &self.preamble
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
