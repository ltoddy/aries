use aries_event::Notifier;
use futures::StreamExt;
use rig_agent::agent::{Agent, AgentHook, MultiTurnStreamItem, PromptResponse};
use rig_agent::completion::{CompletionModel, Message};
use rig_agent::streaming::StreamingPrompt;

use crate::{AriesError, AriesResult};

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

#[derive(Clone)]
pub struct AriesAgent<M>
where
    M: CompletionModel,
{
    inner: Agent<M>,
    name: String,
    preamble: String,
    notifier: Notifier,
}

impl<M> AriesAgent<M>
where
    M: CompletionModel + 'static,
{
    pub fn new(
        inner: Agent<M>,
        name: impl Into<String>,
        preamble: impl Into<String>,
        notifier: Notifier,
    ) -> Self {
        let name = name.into();
        let bare_preamble = preamble.into();

        Self { inner, name, preamble: bare_preamble, notifier }
    }

    pub async fn prompt<I, T, P>(
        &self,
        prompt: impl Into<Message> + Send,
        history: I,
        hook: P,
    ) -> AriesResult<PromptResponse>
    where
        I: IntoIterator<Item = T>,
        T: Into<Message>,
        P: AgentHook + 'static,
    {
        let stream = self.inner.stream_prompt(prompt).history(history).add_hook(hook).await;
        tokio::pin!(stream);

        let mut final_res = PromptResponse::empty();
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(item) => {
                    self.notifier.send_stream_item(item.clone());

                    if let MultiTurnStreamItem::FinalResponse(res) = item {
                        final_res = res;
                    }
                },
                Err(err) => return Err(AriesError::Streaming(err)),
            }
        }

        Ok(final_res)
    }

    pub fn preamble(&self) -> &str {
        &self.preamble
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
