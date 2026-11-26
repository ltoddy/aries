mod agent;
mod breaker;
mod error;
mod micro_compact;
#[cfg(test)]
mod tests;
mod tokens;
mod window;

use aries_communicate::{CompactionStatus, Notifier};
use nanoid::nanoid;
use rig::Model;
use rig::completion::{Message, Usage};
use rig::providers::openai::wire::OpenAiWire;

pub use self::agent::{CompactAgent, CompactOutcome, compact_summary};
pub use self::breaker::{AutoCompactBreaker, Decision};
pub use self::error::{CompactionError, CompactionFailure};
pub use self::micro_compact::{KEEP_RECENT, micro_compact};
pub use self::tokens::TokenEstimator;
pub use self::window::ContextWindow;
use crate::context::ChatContext;
use crate::state::State;

#[derive(Clone)]
pub struct ContextCompactor {
    agent: CompactAgent,
    breaker: AutoCompactBreaker,
    chat_context: ChatContext,
    notifier: Notifier,
    window: ContextWindow,
}

impl ContextCompactor {
    pub fn new(state: State, chat_context: ChatContext, notifier: Notifier) -> Self {
        let model = state.model();
        let vendor = model.wire.provider().dialect.name;
        let model = model.name();
        let window = ContextWindow::new(vendor, model);

        let breaker = AutoCompactBreaker::new();

        let agent = CompactAgent::new(state.model(), state.transcript_dir());

        Self { agent, breaker, chat_context, notifier, window }
    }

    pub fn set_model(&mut self, model: Model<OpenAiWire>) {
        self.agent.set_model(model);
    }

    pub async fn pre_compact(&mut self, prompt: &Message) {
        let mut history = self.chat_context.history().await;
        if micro_compact(&mut history, KEEP_RECENT) {
            self.chat_context.overwrite(history).await;

            let id = format!("micro-compression-{}", nanoid!());
            self.notifier.send_compaction(id, CompactionStatus::Completed, "上下文微压缩已完成");
        }

        let compact_threshold = self.window.auto_compact_threshold();

        let estimated_tokens = {
            let read = self.chat_context.history().await;
            read.estimate_tokens().saturating_add(prompt.estimate_tokens())
        };

        if estimated_tokens >= compact_threshold {
            let compaction_id = format!("abstract-compression-{}", nanoid!());
            let text = format!(
                "预估 tokens {estimated_tokens} 已达到摘要压缩阈值 {compact_threshold}（上下文窗口 {}），开始自动摘要压缩。",
                self.window.context_window
            );
            self.notifier.send_compaction(&compaction_id, CompactionStatus::InProgress, text);
            match self.compact().await {
                Ok(_) => {
                    self.notifier.send_compaction(
                        &compaction_id,
                        CompactionStatus::Completed,
                        "上下文摘要压缩已完成。",
                    );
                },
                Err(err) => {
                    self.notifier.send_compaction(
                        &compaction_id,
                        CompactionStatus::Failed,
                        err.to_string(),
                    );
                },
            }
        }
    }

    pub async fn post_compact(&mut self, usage: Usage) {
        let compact_threshold = self.window.auto_compact_threshold();

        if let Some(total_tokens) = usage.total_tokens
            && total_tokens > compact_threshold
        {
            let compaction_id = format!("abstract-compression-{}", nanoid!());

            let text = format!(
                "实际 tokens {total_tokens} 已超过摘要压缩阈值 {compact_threshold}，开始自动摘要压缩。",
            );
            self.notifier.send_compaction(&compaction_id, CompactionStatus::InProgress, text);
            match self.compact().await {
                Ok(_) => {
                    self.notifier.send_compaction(
                        &compaction_id,
                        CompactionStatus::Completed,
                        "上下文摘要压缩已完成。",
                    );
                },
                Err(err) => {
                    self.notifier.send_compaction(
                        &compaction_id,
                        CompactionStatus::Failed,
                        err.to_string(),
                    );
                },
            }
        }
    }

    pub async fn compact(&mut self) -> Result<String, CompactionError> {
        match self.breaker.decide() {
            Decision::Allow { half_open: _ } => {},
            Decision::Skip { wait, consecutive_failures } => {
                return Err(CompactionError::cooldown(wait, consecutive_failures));
            },
        }

        let outcome = {
            let read = self.chat_context.history().await;
            self.agent.compact(&read).await
        };

        match outcome {
            CompactOutcome::Success((compressed, summary)) => {
                self.chat_context.overwrite(compressed).await;
                self.breaker.on_success();
                Ok(summary)
            },
            CompactOutcome::PromptTooLong => {
                self.breaker.trip();
                Err(CompactionError::prompt_too_long())
            },
            CompactOutcome::Transient(err) => Err(CompactionError::transient(err)),
            CompactOutcome::Empty => {
                self.breaker.on_failure();
                let failures = self.breaker.consecutive_failures();
                if failures >= AutoCompactBreaker::MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES {
                    return Err(CompactionError::too_many_failures(
                        failures,
                        AutoCompactBreaker::AUTOCOMPACT_FAILURE_COOLDOWN.as_secs() / 60,
                    ));
                }
                Err(CompactionError::empty())
            },
        }
    }
}
