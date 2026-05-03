pub mod agent_type;
pub mod builder;
pub mod compaction;
pub mod ext;
pub mod fs;
pub mod jsonl;
pub mod language_server;
pub mod preamble;
pub mod rpc;
pub mod task_spawner;
pub mod tools;

pub use builder::AgentBuilder;
use rig::agent::{Agent, PromptHook, StreamingResult};
use rig::completion::{self, Message, Prompt};
use rig::streaming::StreamingPrompt;

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

pub struct AriesAgent<M, P = ()>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    inner: Agent<M, P>,
    preamble: String,
}

impl<M, P> AriesAgent<M, P>
where
    M: completion::CompletionModel,
    P: PromptHook<M>,
{
    pub fn new(inner: Agent<M, P>, preamble: String) -> Self {
        Self { inner, preamble }
    }
}

impl<M, P> AriesAgent<M, P>
where
    M: completion::CompletionModel + 'static,
    P: PromptHook<M> + 'static,
{
    pub async fn stream_prompt(
        &mut self,
        prompt: &str,
        history: &[Message],
    ) -> StreamingResult<<M>::StreamingResponse> {
        self.inner.stream_prompt(prompt).with_history(history.to_vec()).await
    }

    pub async fn prompt(&mut self, prompt: &str, history: &[Message]) -> anyhow::Result<String> {
        let res = self.inner.prompt(prompt).with_history(history.to_vec()).await?;
        Ok(res)
    }

    #[inline]
    pub fn system_prompt(&self) -> &str {
        &self.preamble
    }
}
