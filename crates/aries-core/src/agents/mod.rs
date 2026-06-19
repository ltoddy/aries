pub mod builder;
pub mod compact;
pub mod mode;
pub mod summary;
pub mod title;

use futures::StreamExt;
use rig_core::agent::{Agent, FinalResponse, MultiTurnStreamItem, PromptHook, StreamingResult};
use rig_core::completion::{CompletionModel, Message};
use rig_core::streaming::StreamingPrompt;
use rig_core::wasm_compat::WasmCompatSend;
use tokio::sync::mpsc::UnboundedSender;

pub use crate::agents::builder::AgentBuilder;
pub use crate::agents::compact::{CompactAgent, CompactOutcome};
pub use crate::agents::mode::Mode;
pub use crate::agents::summary::SummaryAgent;
pub use crate::agents::title::TitleAgent;
use crate::event::AgentEvent;
use crate::{AriesError, AriesResult};

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

#[derive(Clone)]
pub struct AriesAgent<M>
where
    M: CompletionModel,
{
    inner: Agent<M>,

    preamble: String,
    name: String,

    sender: Option<UnboundedSender<AgentEvent>>,
}

impl<M> AriesAgent<M>
where
    M: CompletionModel + 'static,
{
    pub fn new(
        inner: Agent<M>,
        name: impl Into<String>,
        preamble: impl Into<String>,
        sender: Option<UnboundedSender<AgentEvent>>,
    ) -> Self {
        let name = name.into();
        let preamble = preamble.into();

        Self { inner, preamble, name, sender }
    }

    pub async fn prompt<I, T, P>(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: I,
        hook: P,
    ) -> AriesResult<FinalResponse>
    where
        I: IntoIterator<Item = T>,
        T: Into<Message>,
        P: PromptHook<M> + 'static,
    {
        let stream = self.inner.stream_prompt(prompt).with_history(history).with_hook(hook).await;
        tokio::pin!(stream);

        let mut final_res = FinalResponse::empty();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(item) => {
                    if let Some(ref sender) = self.sender {
                        let event = AgentEvent::from_stream(true, self.name.clone(), item.clone());
                        let _ = sender.send(event);
                    }

                    if let MultiTurnStreamItem::FinalResponse(res) = item {
                        final_res = res;
                    }
                },
                Err(err) => return Err(AriesError::Streaming(err)),
            }
        }

        Ok(final_res)
    }

    pub async fn stream_prompt<I, T>(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: I,
    ) -> StreamingResult<<M>::StreamingResponse>
    where
        I: IntoIterator<Item = T>,
        T: Into<Message>,
    {
        self.inner.stream_prompt(prompt).with_history(history).await
    }

    pub fn system_prompt(&self) -> &str {
        &self.preamble
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
