pub mod agent_type;
pub mod builder;
pub mod compaction;
pub mod summary;
pub mod title;

use futures::StreamExt;
use rig::agent::{Agent, FinalResponse, MultiTurnStreamItem, PromptHook, StreamingResult};
use rig::completion::{CompletionModel, Message};
use rig::streaming::StreamingPrompt;
use rig::wasm_compat::WasmCompatSend;

pub use self::agent_type::AgentType;
pub use self::builder::AgentBuilder;
pub use self::compaction::CompactionAgent;
pub use self::summary::SummaryAgent;
pub use self::title::TitleAgent;

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

#[derive(Clone)]
pub struct AriesAgent<M, P = ()>
where
    M: CompletionModel,
    P: PromptHook<M>,
{
    inner: Agent<M, P>,

    preamble: String,
    name: String,
}

impl<M, P> AriesAgent<M, P>
where
    M: CompletionModel,
    P: PromptHook<M>,
{
    pub fn new(inner: Agent<M, P>, name: impl Into<String>, preamble: impl Into<String>) -> Self {
        let name = name.into();
        let preamble = preamble.into();

        Self { inner, preamble, name }
    }
}

impl<M> AriesAgent<M>
where
    M: CompletionModel + 'static,
{
    pub async fn stream_prompt<P>(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: &[Message],
        hook: P,
    ) -> StreamingResult<<M>::StreamingResponse>
    where
        P: PromptHook<M> + 'static,
    {
        self.inner.stream_prompt(prompt).with_history(history).with_hook(hook).await
    }

    pub async fn completion(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: &[Message],
    ) -> anyhow::Result<String> {
        let stream = self.inner.stream_prompt(prompt).with_history(history).await;
        futures::pin_mut!(stream);

        let mut final_res = FinalResponse::empty();
        while let Some(item) = stream.next().await {
            let item = item?;

            if let MultiTurnStreamItem::FinalResponse(res) = item {
                final_res = res;
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
