use rig::agent::{
    AgentHook, CompletionCallAction, CompletionCallEvent, HookContext, ModelTurnAction,
    ModelTurnFinished, OutcomeAction, OutcomeEvent, RunSettled,
};
use rig::message::{AssistantMessage, Message};

use crate::context::{ChatContext, ChatHistory};

pub struct TranscriptHook {
    chat_history: ChatHistory,
    chat_context: ChatContext,
}

#[derive(Clone)]
struct TranscriptProgress {
    history_len: usize,
    written: usize,
    recorded: Option<Message>,
}

impl TranscriptProgress {
    fn new(history_len: usize) -> Self {
        Self { history_len, written: 0, recorded: None }
    }
}

impl TranscriptHook {
    pub fn new(chat_history: ChatHistory, chat_context: ChatContext) -> Self {
        Self { chat_history, chat_context }
    }

    async fn append_messages(
        &self,
        ctx: &HookContext,
        messages: &[Message],
        mut progress: TranscriptProgress,
    ) {
        let history = messages
            .iter()
            .filter(|message| {
                if progress.recorded.as_ref() == Some(*message) {
                    progress.recorded.take();
                    false
                } else {
                    true
                }
            })
            .collect::<Vec<_>>();
        if !messages.is_empty() {
            self.chat_history.append(history).await;
            self.chat_context.append(messages).await;
            progress.written += messages.len();
        }
        progress.recorded = None;
        ctx.scratchpad().insert(progress);
    }
}

impl AgentHook for TranscriptHook {
    fn name(&self) -> Option<String> {
        Some(String::from("TranscriptHook"))
    }

    async fn on_run_settled(&self, ctx: &HookContext, event: RunSettled<'_>) {
        if let Some(messages) = event.messages {
            let progress = ctx
                .scratchpad()
                .get::<TranscriptProgress>()
                .unwrap_or_else(|| TranscriptProgress::new(0));
            let messages = messages
                .iter()
                .skip(progress.written)
                .map(|message| message.as_ref().clone())
                .collect::<Vec<Message>>();
            self.append_messages(ctx, &messages, progress).await;
        }
    }

    async fn on_completion_call(
        &self,
        ctx: &HookContext,
        event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        let progress = ctx
            .scratchpad()
            .get::<TranscriptProgress>()
            .unwrap_or_else(|| TranscriptProgress::new(event.history.len()));

        let messages = event
            .history
            .iter()
            .skip(progress.history_len)
            .map(|message| message.as_ref().clone())
            .chain(std::iter::once(event.prompt.clone()))
            .skip(progress.written)
            .collect::<Vec<Message>>();
        self.append_messages(ctx, &messages, progress).await;
        CompletionCallAction::continue_run()
    }

    async fn on_model_turn_finished(
        &self,
        ctx: &HookContext,
        event: ModelTurnFinished<'_>,
    ) -> ModelTurnAction {
        if let Some(head) = ctx.scratchpad().get::<AssistantMessage>() {
            let assistant = head.with_content(event.content.clone());
            if !assistant.is_empty_turn() {
                let message = Message::from(assistant);
                self.chat_history.append(std::iter::once(&message)).await;
                let mut progress = ctx
                    .scratchpad()
                    .get::<TranscriptProgress>()
                    .unwrap_or_else(|| TranscriptProgress::new(0));
                progress.recorded = Some(message);
                ctx.scratchpad().insert(progress);
            }
        }
        ModelTurnAction::continue_run()
    }

    async fn on_outcome(&self, ctx: &HookContext, event: OutcomeEvent<'_>) -> OutcomeAction {
        if let Some(response) = event.completion() {
            ctx.scratchpad().insert(response.head());
        }
        OutcomeAction::proceed()
    }
}
