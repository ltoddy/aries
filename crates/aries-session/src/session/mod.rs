mod history;
mod hook;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use aries_core::agents::{CompactAgent, CompactOutcome, Mode};
use aries_core::compact::{AutoCompactBreaker, Decision, TokenEstimator};
use aries_core::event::AgentEvent;
use aries_core::language_server::{LspServerInfo, SharedLspClient, warm_up};
use aries_core::{agents, compact};
use aries_init::{ModelConfig, Setting, SettingError};
use futures::pin_mut;
use rig_core::agent::FinalResponse;
use rig_core::completion::Message;
use rig_core::wasm_compat::WasmCompatSend;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

use crate::session::history::ChatHistory;
use crate::{AriesAgent, AriesClient};

#[derive(Clone)]
pub struct Session {
    id: String,

    setting: Setting,
    config: ModelConfig,
    cwd: PathBuf,
    client: AriesClient,
    agent: AriesAgent,
    mode: Mode,

    lsp_client: Option<SharedLspClient>,
    chat_history: ChatHistory,

    root_dir: PathBuf,
    transcript_dir: PathBuf,

    cancel: CancellationToken,
    receiver: Arc<AsyncMutex<UnboundedReceiver<AgentEvent>>>,

    compact_breaker: AutoCompactBreaker,
}

impl Session {
    const FILENAME: &str = "chat-history.jsonl";
    pub const PREFIX: &str = "session-";

    pub async fn new(
        id: impl Into<String>,
        root_dir: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
        config: ModelConfig,
        setting: Setting,
    ) -> anyhow::Result<Self> {
        let id = id.into();
        let cwd = cwd.as_ref();

        let client = AriesClient::new(&config)?;
        let lsp_client = Self::warm_up_lsp(cwd).await;

        let root_dir = root_dir.as_ref();
        #[rustfmt::skip]
        tokio::fs::create_dir_all(&root_dir)
            .await
            .with_context(|| format!("failed to create session directory at: {}", root_dir.display()))?;

        let transcript_dir = root_dir.join("transcripts");
        #[rustfmt::skip]
        tokio::fs::create_dir_all(&transcript_dir)
            .await
            .with_context(|| format!("failed to create session transcripts directory at: {}", transcript_dir.display()))?;

        let mode = Mode::default();
        let (agent, receiver) = client.agent(mode, config.clone(), cwd, lsp_client.clone()).await?;

        let chat_history = ChatHistory::new(root_dir.join(Self::FILENAME)).await;
        let cancel = CancellationToken::new();

        Ok(Self {
            id,
            setting,
            config,
            cwd: cwd.to_path_buf(),
            client,
            agent,
            mode,
            lsp_client,
            chat_history,
            root_dir: root_dir.to_path_buf(),
            transcript_dir,
            cancel,
            receiver: Arc::new(AsyncMutex::new(receiver)),
            compact_breaker: AutoCompactBreaker::new(),
        })
    }

    pub async fn load(
        id: String,
        root_dir: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
        config: ModelConfig,
        setting: Setting,
    ) -> anyhow::Result<Self> {
        let cwd = cwd.as_ref();
        let root_dir = root_dir.as_ref();
        let transcript_dir = root_dir.join("transcripts");

        let lsp_client = Self::warm_up_lsp(cwd).await;
        let client = AriesClient::new(&config)?;

        let mode = Mode::default();
        let (agent, agent_events) =
            client.agent(mode, config.clone(), cwd, lsp_client.clone()).await?;
        let chat_history = ChatHistory::new(root_dir.join(Self::FILENAME)).await;
        let cancel = CancellationToken::new();

        Ok(Self {
            id,
            setting,
            config,
            cwd: cwd.to_path_buf(),
            client,
            agent,
            mode,
            lsp_client,
            chat_history,
            root_dir: root_dir.to_path_buf(),
            transcript_dir,
            cancel,
            receiver: Arc::new(AsyncMutex::new(agent_events)),
            compact_breaker: AutoCompactBreaker::new(),
        })
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn system_prompt(&self) -> &str {
        self.agent.system_prompt()
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn setting(&self) -> &Setting {
        &self.setting
    }

    pub fn history(&self) -> &[Message] {
        self.chat_history.history()
    }

    pub fn clear_history(&mut self) {
        self.chat_history.clear();
    }

    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    pub async fn set_model(&mut self, alias: impl Into<String>) -> anyhow::Result<()> {
        let alias = alias.into();

        let config = match self.setting.models.iter().find(|m| m.alias().into() == alias) {
            Some(config) => config,
            None => return Err(SettingError::not_found(alias).into()),
        };
        self.client = AriesClient::new(config)?;
        let (agent, agent_events) = self
            .client
            .agent(self.mode, self.config.clone(), self.cwd.clone(), self.lsp_client.clone())
            .await?;
        self.agent = agent;
        self.receiver = Arc::new(AsyncMutex::new(agent_events));
        self.config = config.to_owned();
        self.setting.active = alias;
        Ok(())
    }

    pub async fn set_mode(&mut self, mode: Mode) -> anyhow::Result<()> {
        let (agent, agent_events) = self
            .client
            .agent(mode, self.config.clone(), self.cwd.clone(), self.lsp_client.clone())
            .await?;

        self.mode = mode;
        self.agent = agent;
        self.receiver = Arc::new(AsyncMutex::new(agent_events));
        Ok(())
    }

    pub fn dir(&self) -> PathBuf {
        self.root_dir.clone()
    }

    pub async fn prompt<F, Fut>(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        mut cb: Option<F>,
    ) -> anyhow::Result<()>
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        let prompt: Message = prompt.into();
        self.cancel = CancellationToken::new();

        let cancel_token = self.cancel.clone();
        let agent_events = self.receiver.clone();

        compact::micro_compact(self.chat_history.history_mut());

        let window = compact::ContextWindow::for_model(self.config.model().into());
        let compact_threshold = window.auto_compact_threshold();
        let estimate_tokens =
            self.chat_history.history().estimate_tokens().saturating_add(prompt.estimate_tokens());

        if estimate_tokens >= compact_threshold {
            println!(
                "\n🔄 预估 tokens {} 已达阈值 {}（上下文窗口 {}），提前触发压缩...",
                estimate_tokens, compact_threshold, window.total
            );
            self.compact().await;
        }

        let final_res = match &mut self.agent {
            AriesAgent::Azure(agent) => {
                let snapshot = self.chat_history.history().to_vec();
                Self::run_prompt(
                    agent,
                    prompt,
                    &snapshot,
                    &mut cb,
                    agent_events.clone(),
                    cancel_token,
                )
                .await?
            },
            AriesAgent::Deepseek(agent) => {
                let snapshot = self.chat_history.history().to_vec();
                Self::run_prompt(
                    agent,
                    prompt,
                    &snapshot,
                    &mut cb,
                    agent_events.clone(),
                    cancel_token,
                )
                .await?
            },
            AriesAgent::OpenAI(agent) => {
                let snapshot = self.chat_history.history().to_vec();
                Self::run_prompt(
                    agent,
                    prompt,
                    &snapshot,
                    &mut cb,
                    agent_events.clone(),
                    cancel_token,
                )
                .await?
            },
        };

        if let Some(his) = final_res.history() {
            self.chat_history.extend(his.iter().cloned());
            self.chat_history.persist();
        }

        if final_res.usage().total_tokens > compact_threshold {
            println!(
                "\n🔄 实际 tokens {} 已达阈值 {}，触发压缩...",
                final_res.usage().total_tokens,
                compact_threshold
            );
            self.compact().await;
        }

        Ok(())
    }

    pub async fn compact(&mut self) -> bool {
        match self.compact_breaker.decide() {
            Decision::Skip { wait, consecutive_failures } => {
                println!(
                    "\n⏳ 压缩处于冷却期（已连续失败 {} 次），约 {} 秒后重试，本次跳过。",
                    consecutive_failures,
                    wait.as_secs(),
                );
                return false;
            },
            Decision::Allow { half_open } => {
                if half_open {
                    println!("\n🔁 冷却结束，尝试恢复压缩...");
                }
            },
        }

        let outcome = match self.client.clone() {
            AriesClient::Azure(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcript_dir);
                compact_agent.compact(self.chat_history.history()).await
            },
            AriesClient::Deepseek(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcript_dir);
                compact_agent.compact(self.chat_history.history()).await
            },
            AriesClient::OpenAI(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcript_dir);
                compact_agent.compact(self.chat_history.history()).await
            },
        };

        match outcome {
            CompactOutcome::Success(compressed) => {
                self.chat_history.reset(&compressed);

                let window = compact::ContextWindow::for_model(self.config.model());
                let post_tokens = self.chat_history.history().estimate_tokens();
                let threshold = window.auto_compact_threshold();
                if post_tokens >= threshold {
                    println!(
                        "\n⚠️  压缩后 tokens {} 仍高于阈值 {}，本次压缩无效。",
                        post_tokens, threshold,
                    );
                    self.compact_breaker.on_failure();
                    return false;
                }

                self.compact_breaker.on_success();
                true
            },
            CompactOutcome::Transient(err) => {
                println!("\n🌐 压缩遇到临时错误（不计入失败）：{}", err);
                false
            },
            CompactOutcome::PromptTooLong => {
                println!("\n🛑 上下文过长，压缩请求被拒，进入冷却以避免反复重试。");
                self.compact_breaker.trip();
                false
            },
            CompactOutcome::Empty => {
                self.compact_breaker.on_failure();
                let failures = self.compact_breaker.consecutive_failures();
                if failures >= AutoCompactBreaker::MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES {
                    println!(
                        "\n⛔ 连续 {} 次压缩失败，进入 {} 分钟冷却。",
                        failures,
                        AutoCompactBreaker::AUTOCOMPACT_FAILURE_COOLDOWN.as_secs() / 60,
                    );
                }
                false
            },
        }
    }

    async fn run_prompt<M, F, Fut>(
        agent: &mut agents::AriesAgent<M>,
        prompt: impl Into<Message> + WasmCompatSend,
        history: &[Message],
        cb: &mut Option<F>,
        agent_events: Arc<AsyncMutex<UnboundedReceiver<AgentEvent>>>,
        cancel_token: CancellationToken,
    ) -> anyhow::Result<FinalResponse>
    where
        M: rig_core::completion::CompletionModel + 'static,
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        let prompt_fut = agent.prompt(prompt, history.to_vec());
        pin_mut!(prompt_fut);

        let mut events_guard = agent_events.lock().await;
        let mut final_res = FinalResponse::empty();

        loop {
            tokio::select! {
                biased;
                _ = cancel_token.cancelled() => {
                    break;
                }
                event = events_guard.recv() => {
                    match event {
                        Some(event) => {
                            if let Some(cb) = cb {
                                cb(event).await;
                            }
                        }
                        None => {
                            // sender side dropped; ignore further agent events
                        }
                    }
                }
                res = &mut prompt_fut => {
                    final_res = res.map_err(|e| anyhow::anyhow!(e))?;
                    break;
                }
            }
        }

        // Drain any remaining buffered agent events without blocking
        while let Ok(event) = events_guard.try_recv() {
            if let Some(cb) = cb {
                cb(event).await;
            }
        }

        Ok(final_res)
    }

    async fn warm_up_lsp(dir: impl AsRef<Path>) -> Option<SharedLspClient> {
        let dir = dir.as_ref();

        if let Some(info) = LspServerInfo::detect(dir)
            && info.installed()
            && let Ok(lsp) = warm_up(info, dir).await
        {
            return Some(lsp);
        }

        None
    }
}

#[inline]
fn sanitize_dir(dir: impl AsRef<Path>) -> String {
    dir.as_ref()
        .display()
        .to_string()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}
