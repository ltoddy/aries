use std::io;
use std::path::Path;

use rig::agent::{AgentHook, CompletionCallAction, CompletionCallEvent, HookContext, RunSettled};
use rig::message::Message;

use crate::session::ChatHistory;

pub struct TranscriptHook {
    chat_history: ChatHistory,
}

impl TranscriptHook {
    pub async fn new(dir: impl AsRef<Path>) -> io::Result<Self> {
        let chat_history = ChatHistory::new(dir).await?;
        Ok(Self { chat_history })
    }
}

impl AgentHook for TranscriptHook {
    async fn on_run_settled(&self, ctx: &HookContext, event: RunSettled<'_>) {
        if let Some(messages) = event.messages {
            let (history_len, written) = ctx.scratchpad().get::<(usize, usize)>().unwrap_or((0, 0));
            let messages = messages
                .iter()
                .skip(written)
                .map(|message| message.as_ref().clone())
                .collect::<Vec<Message>>();
            if !messages.is_empty() {
                self.chat_history.append(&messages).await;
                ctx.scratchpad().insert((history_len, written + messages.len()));
            }
        }
    }

    async fn on_completion_call(
        &self,
        ctx: &HookContext,
        event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        let (history_len, written) =
            ctx.scratchpad().get::<(usize, usize)>().unwrap_or((event.history.len(), 0));
        ctx.scratchpad().insert((history_len, written));

        let messages = event
            .history
            .iter()
            .skip(history_len)
            .map(|message| message.as_ref().clone())
            .chain(std::iter::once(event.prompt.clone()))
            .skip(written)
            .collect::<Vec<Message>>();
        if !messages.is_empty() {
            self.chat_history.append(&messages).await;
            ctx.scratchpad().insert((history_len, written + messages.len()));
        }
        CompletionCallAction::continue_run()
    }
}
