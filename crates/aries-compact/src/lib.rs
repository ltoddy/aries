mod agent;
mod breaker;
mod error;
mod micro_compact;
#[cfg(test)]
mod tests;
mod tokens;
mod window;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use aries_context::ChatContext;
use aries_event::{CompactionStatus, Notifier};
use aries_extension::hook::input::{
    PostCompactHookInput, PostCompactTrigger, PreCompactCustomInstructions, PreCompactHookInput,
};
use aries_extension::hook::{HookDecision, HooksExecutor};
use nanoid::nanoid;
use rig::Model;
use rig::completion::{Message, Usage};
use rig::providers::openai::wire::OpenAiWire;

pub use crate::agent::{CompactAgent, CompactOutcome, compact_summary};
pub use crate::breaker::{AutoCompactBreaker, Decision};
pub use crate::error::CompactionError;
pub use crate::micro_compact::{KEEP_RECENT, micro_compact};
pub use crate::tokens::TokenEstimator;
pub use crate::window::ContextWindow;

#[derive(Clone)]
pub struct ContextCompactor {
    id: String,
    cwd: PathBuf,
    agent: CompactAgent,
    transcript_path: PathBuf,
    breaker: AutoCompactBreaker,
    chat_context: ChatContext,
    hooks_executor: Arc<HooksExecutor>,
    notifier: Notifier,
}

impl ContextCompactor {
    pub fn new(
        id: impl Into<String>,
        cwd: impl AsRef<Path>,
        transcript_path: impl AsRef<Path>,
        model: Model<OpenAiWire>,
        chat_context: ChatContext,
        hooks_executor: Arc<HooksExecutor>,
        notifier: Notifier,
    ) -> Self {
        let id = id.into();
        let cwd = cwd.as_ref();
        let transcript_path = transcript_path.as_ref();
        let breaker = AutoCompactBreaker::new();
        let agent = CompactAgent::new(model, transcript_path);

        Self {
            agent,
            transcript_path: transcript_path.to_owned(),
            breaker,
            chat_context,
            hooks_executor,
            id,
            cwd: cwd.to_owned(),
            notifier,
        }
    }

    pub fn set_model(&mut self, model: Model<OpenAiWire>) {
        self.agent.set_model(model);
    }

    pub async fn pre_compact(&mut self, prompt: &Message) {
        let mut history = self.chat_context.history().await.clone();
        if micro_compact(&mut history, KEEP_RECENT) {
            self.chat_context.overwrite(history).await;
        }

        let window = ContextWindow::new();
        let compact_threshold = window.auto_compact_threshold();

        let estimated_tokens = {
            let read = self.chat_context.history().await;
            read.estimate_tokens().saturating_add(prompt.estimate_tokens())
        };

        if estimated_tokens >= compact_threshold {
            let compaction_id = format!("micra-compression-{}", nanoid!());
            let text = format!(
                "预估 tokens {estimated_tokens} 已达到摘要压缩阈值 {compact_threshold}（上下文窗口 {}），开始自动摘要压缩。",
                window.total
            );
            self.notifier.send_compaction(&compaction_id, CompactionStatus::InProgress, text);
            match self.compact().await {
                Ok(Some(_)) => {
                    self.notifier.send_compaction(
                        &compaction_id,
                        CompactionStatus::Completed,
                        "上下文摘要压缩已完成。",
                    );
                },
                Ok(None) => {
                    self.notifier.send_compaction(
                        &compaction_id,
                        CompactionStatus::Cancelled,
                        "上下文摘要压缩被 hook 阻止，已跳过本次压缩。",
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
        let window = ContextWindow::new();
        let compact_threshold = window.auto_compact_threshold();

        if let Some(total_tokens) = usage.total_tokens
            && total_tokens > compact_threshold
        {
            let compaction_id = format!("abstract-compression-{}", nanoid!());

            let text = format!(
                "实际 tokens {total_tokens} 已超过摘要压缩阈值 {compact_threshold}，开始自动摘要压缩。",
            );
            self.notifier.send_compaction(&compaction_id, CompactionStatus::InProgress, text);
            match self.compact().await {
                Ok(Some(_)) => {
                    self.notifier.send_compaction(
                        &compaction_id,
                        CompactionStatus::Completed,
                        "上下文摘要压缩已完成。",
                    );
                },
                Ok(None) => {
                    self.notifier.send_compaction(
                        &compaction_id,
                        CompactionStatus::Cancelled,
                        "上下文摘要压缩被 hook 阻止，已跳过本次压缩。",
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

    pub async fn compact(&mut self) -> Result<Option<String>, CompactionError> {
        match self.breaker.decide() {
            Decision::Allow { half_open: _ } => {},
            Decision::Skip { wait, consecutive_failures } => {
                return Err(CompactionError::cooldown(wait, consecutive_failures));
            },
        }

        let input = PreCompactHookInput::new(
            &self.id,
            &self.cwd,
            PostCompactTrigger::Auto,
            PreCompactCustomInstructions::Auto,
        )
        .transcript_path(&self.transcript_path);
        if let HookDecision::Terminate { reason: _ } =
            self.hooks_executor.fire_pre_compact(input).await
        {
            return Ok(None);
        }

        let outcome = {
            let read = self.chat_context.history().await.clone();
            self.agent.compact(&read).await
        };

        match outcome {
            CompactOutcome::Success((compressed, summary)) => {
                self.chat_context.overwrite(compressed).await;
                self.breaker.on_success();

                let input = PostCompactHookInput::new(
                    &self.id,
                    &self.cwd,
                    PostCompactTrigger::Auto,
                    &summary,
                )
                .transcript_path(&self.transcript_path);
                self.hooks_executor.fire_post_compact(input).await;
                Ok(Some(summary))
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
