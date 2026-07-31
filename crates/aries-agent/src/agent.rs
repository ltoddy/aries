use aries_event::AgentEvent;
use aries_mode::Mode;
use futures::StreamExt;
use rig_agent::agent::{Agent, AgentHook, MultiTurnStreamItem, PromptResponse};
use rig_agent::completion::{CompletionModel, Message};
use rig_agent::streaming::StreamingPrompt;
use tokio::sync::mpsc::UnboundedSender;

use crate::{AriesError, AriesResult};

pub const AGENT_LOOP_MAX_TURNS: usize = 200;

#[derive(Clone)]
pub struct AriesAgent<M>
where
    M: CompletionModel,
{
    inner: Agent<M>,
    name: String,
    bare_preamble: String,
    sections: Vec<String>,
    sender: Option<UnboundedSender<AgentEvent>>,
}

impl<M> AriesAgent<M>
where
    M: CompletionModel + 'static,
{
    pub fn new(
        inner: Agent<M>,
        name: impl Into<String>,
        bare_preamble: impl Into<String>,
        sections: &[String],
        sender: Option<UnboundedSender<AgentEvent>>,
    ) -> Self {
        let name = name.into();
        let bare_preamble = bare_preamble.into();
        let sections = sections.to_vec();

        Self { inner, name, bare_preamble, sections, sender }
    }

    pub async fn prompt<I, T, P>(
        &mut self,
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

    pub fn set_mode(&mut self, mode: Mode) {
        let bare_preamble = mode.bare_preamble();

        let mut preamble = String::new();
        preamble.push_str(bare_preamble);
        preamble.push('\n');
        for section in &self.sections {
            preamble.push('\n');
            preamble.push_str(section);
        }

        self.bare_preamble = bare_preamble.to_owned();
        // self.inner.preamble = Some(preamble);
    }

    pub fn system_prompt(&self) -> String {
        // let preamble = self.inner.preamble.clone();
        // preamble.unwrap_or_default()
        String::new()
    }
}
