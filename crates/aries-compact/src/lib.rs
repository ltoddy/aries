mod agent;
mod breaker;
mod micro_compact;
mod tokens;
mod window;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aries_context::ChatContext;
use aries_event::{AgentEvent, Notifier};
use aries_extension::hook::input::{
    PostCompactHookInput, PostCompactTrigger, PreCompactCustomInstructions, PreCompactHookInput,
};
use aries_extension::hook::{HookDecision, HooksExecutor};
use rig::completion::{Message, Usage};

pub use self::agent::{CompactAgent, CompactOutcome, compact_summary};
pub use self::breaker::{AutoCompactBreaker, Decision};
pub use self::micro_compact::{KEEP_RECENT, micro_compact};
pub use self::tokens::TokenEstimator;
pub use self::window::ContextWindow;

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
        agent: CompactAgent,
        chat_context: ChatContext,
        hooks_executor: Arc<HooksExecutor>,
        notifier: Notifier,
    ) -> Self {
        let id = id.into();
        let cwd = cwd.as_ref();
        let transcript_path = transcript_path.as_ref();
        let breaker = AutoCompactBreaker::new();

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

    pub fn set_agent(&mut self, agent: CompactAgent) {
        self.agent = agent;
    }

    pub async fn pre_compact<F, Fut>(&mut self, prompt: &Message, mut callback: F)
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        {
            let mut write = self.chat_context.history_mut().await;
            micro_compact(&mut write, KEEP_RECENT);
        }

        let window = ContextWindow::new();
        let compact_threshold = window.auto_compact_threshold();

        let estimated_tokens = {
            let read = self.chat_context.history().await;
            read.estimate_tokens().saturating_add(prompt.estimate_tokens())
        };

        if estimated_tokens >= compact_threshold {
            let text = format!(
                "\n预估 tokens {estimated_tokens} 已达阈值 {compact_threshold}（上下文窗口 {}），提前触发压缩...\n",
                window.total
            );
            callback(AgentEvent::notification(text)).await;
            self.compact().await;
        }
    }

    pub async fn post_compact<F, Fut>(&mut self, usage: Usage, mut callback: F)
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        let window = ContextWindow::new();
        let compact_threshold = window.auto_compact_threshold();

        if usage.total_tokens > compact_threshold {
            let text = format!(
                "\n实际 tokens {} 已达阈值 {compact_threshold}，触发压缩...\n",
                usage.total_tokens,
            );
            callback(AgentEvent::notification(text)).await;
            self.compact().await;
        }
    }

    pub async fn compact(&mut self) {
        match self.breaker.decide() {
            Decision::Allow { half_open } => {
                if half_open {
                    self.notifier.notify("冷却结束，尝试恢复压缩...");
                }
            },
            Decision::Skip { wait, consecutive_failures } => {
                self.notifier.notify(format!("压缩处于冷却期（已连续失败 {consecutive_failures} 次），约 {wait:?} 后重试，本次跳过。"));
                return;
            },
        }

        let input = PreCompactHookInput::new(
            &self.id,
            &self.cwd,
            PostCompactTrigger::Auto,
            PreCompactCustomInstructions::Auto,
        )
        .transcript_path(&self.transcript_path);
        if let HookDecision::Terminate { .. } = self.hooks_executor.fire_pre_compact(input).await {
            return;
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
                    summary,
                )
                .transcript_path(&self.transcript_path);
                self.hooks_executor.fire_post_compact(input).await;
            },
            CompactOutcome::PromptTooLong => {
                self.notifier.notify("上下文过长，压缩请求被拒，进入冷却以避免反复重试。");
                self.breaker.trip();
            },
            CompactOutcome::Transient(err) => {
                self.notifier.notify(format!("压缩遇到临时错误（不计入失败）：{err}"));
            },
            CompactOutcome::Empty => {
                self.breaker.on_failure();
                let failures = self.breaker.consecutive_failures();
                if failures >= AutoCompactBreaker::MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES {
                    self.notifier.notify(format!(
                        "连续 {failures} 次压缩失败，进入 {} 分钟冷却。",
                        AutoCompactBreaker::AUTOCOMPACT_FAILURE_COOLDOWN.as_secs() / 60,
                    ));
                }
            },
        }
    }
}
