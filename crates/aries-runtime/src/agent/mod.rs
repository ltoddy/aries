mod builder;
mod error;

use aries_communicate::Notifier;
use futures::StreamExt;
use rig::agent::{MultiTurnStreamItem, PromptResponse};
use rig::completion::Message;
use rig::providers::openai::wire::OpenAiWire;
use rig::{Agent, Model};

pub use self::builder::AgentBuilder;
pub use self::error::AriesError;

pub type AriesResult<T, E = AriesError> = Result<T, E>;

#[derive(Clone)]
pub struct AriesAgent {
    inner: Agent,
    preamble: String,
    notifier: Notifier,
}

impl AriesAgent {
    pub fn new(inner: Agent, preamble: impl Into<String>, notifier: Notifier) -> Self {
        let bare_preamble = preamble.into();

        Self { inner, preamble: bare_preamble, notifier }
    }

    pub async fn prompt<I, T>(
        &self,
        prompt: impl Into<Message> + Send,
        history: I,
    ) -> AriesResult<PromptResponse>
    where
        I: IntoIterator<Item = T>,
        T: Into<Message>,
    {
        let stream = self.inner.prompt(prompt).history(history).stream();
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
                Err(err) => return Err(err.into()),
            }
        }

        Ok(final_res)
    }

    pub fn preamble(&self) -> &str {
        &self.preamble
    }

    pub fn set_model(&mut self, model: Model<OpenAiWire>) {
        self.inner.set_model(model);
    }
}
