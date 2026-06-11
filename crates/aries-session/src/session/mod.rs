mod history;
mod hook;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use aries_config::AriesConfig;
use aries_core::agents::{AgentBuilder, AgentType, AriesAgent, CompactAgent, CompactOutcome};
use aries_core::compact::{AutoCompactBreaker, Decision, TokenEstimator};
use aries_core::event::AgentEvent;
use aries_core::language_server::{LspServerInfo, SharedLspClient, warm_up};
use aries_core::{AriesClient, compact};
use futures::pin_mut;
use rig_core::agent::FinalResponse;
use rig_core::completion::Message;
use rig_core::providers::{azure, deepseek, openai};
use rig_core::wasm_compat::WasmCompatSend;
use tokio::sync::Mutex as AsyncMutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

use crate::session::history::ChatHistory;

#[derive(Clone)]
pub struct Session {
    id: String,
    config: AriesConfig,
    cwd: PathBuf,
    client: AriesClient,
    agents: ProviderAgents,
    lsp_client: Option<SharedLspClient>,
    chat_history: ChatHistory,
    root_dir: PathBuf,
    transcript_dir: PathBuf,
    cancel: CancellationToken,
    agent_events: Arc<AsyncMutex<UnboundedReceiver<AgentEvent>>>,
    compact_breaker: AutoCompactBreaker,
}

#[derive(Clone)]
enum ProviderAgents {
    OpenAICompatible { agent: AriesAgent<openai::CompletionModel> },
    Azure { agent: AriesAgent<azure::CompletionModel> },
    DeepSeek { agent: AriesAgent<deepseek::CompletionModel> },
}

impl Session {
    const FILENAME: &str = "chat-history.jsonl";
    pub const PREFIX: &str = "session-";

    pub async fn new(
        id: impl Into<String>,
        config: AriesConfig,
        root_dir: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        let id = id.into();
        let cwd = cwd.as_ref();

        let client = aries_core::create_client(config.clone())?;
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

        let project_dir = root_dir.join(sanitize_dir(cwd));

        let (agents, agent_events) =
            Self::create_agents(AgentType::Build, config.clone(), cwd, lsp_client.clone()).await?;

        let chat_history = ChatHistory::new(root_dir.join(Self::FILENAME)).await;
        let cancel = CancellationToken::new();

        Ok(Self {
            id,
            config,
            cwd: cwd.to_path_buf(),
            client,
            agents,
            lsp_client,
            chat_history,
            root_dir: root_dir.to_path_buf(),
            transcript_dir,
            cancel,
            agent_events: Arc::new(AsyncMutex::new(agent_events)),
            compact_breaker: AutoCompactBreaker::new(),
        })
    }

    pub async fn load(
        id: String,
        config: AriesConfig,
        root_dir: impl AsRef<Path>,
        cwd: impl AsRef<Path>,
    ) -> anyhow::Result<Self> {
        let cwd = cwd.as_ref();
        let root_dir = root_dir.as_ref();
        let transcript_dir = root_dir.join("transcripts");

        let lsp_client = Self::warm_up_lsp(cwd).await;
        let client = aries_core::create_client(config.clone())?;

        let (agents, agent_events) =
            Self::create_agents(AgentType::Build, config.clone(), cwd, lsp_client.clone()).await?;
        let chat_history = ChatHistory::new(root_dir.join(Self::FILENAME)).await;
        let cancel = CancellationToken::new();

        Ok(Self {
            id,
            config,
            cwd: cwd.to_path_buf(),
            client,
            agents,
            lsp_client,
            chat_history,
            root_dir: root_dir.to_path_buf(),
            transcript_dir,
            cancel,
            agent_events: Arc::new(AsyncMutex::new(agent_events)),
            compact_breaker: AutoCompactBreaker::new(),
        })
    }

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn system_prompt(&self) -> &str {
        match &self.agents {
            ProviderAgents::OpenAICompatible { agent, .. } => agent.system_prompt(),
            ProviderAgents::Azure { agent, .. } => agent.system_prompt(),
            ProviderAgents::DeepSeek { agent, .. } => agent.system_prompt(),
        }
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

    pub async fn switch_agent(&mut self, agent_type: AgentType) -> anyhow::Result<()> {
        let (agents, agent_events) = Self::create_agents(
            agent_type,
            self.config.clone(),
            self.cwd.clone(),
            self.lsp_client.clone(),
        )
        .await?;
        self.agents = agents;
        self.agent_events = Arc::new(AsyncMutex::new(agent_events));
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
        let agent_events = self.agent_events.clone();

        compact::micro_compact(self.chat_history.history_mut());

        let window = compact::ContextWindow::for_model(self.config.model());
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

        let final_res = match &mut self.agents {
            ProviderAgents::OpenAICompatible { agent } => {
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
            ProviderAgents::Azure { agent } => {
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
            ProviderAgents::DeepSeek { agent } => {
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
            AriesClient::OpenAI(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcript_dir);
                compact_agent.compact(self.chat_history.history()).await
            },
            AriesClient::Azure(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcript_dir);
                compact_agent.compact(self.chat_history.history()).await
            },
            AriesClient::DeepSeek(client) => {
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
        agent: &mut AriesAgent<M>,
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

    async fn create_agents(
        agent_type: AgentType,
        config: AriesConfig,
        cwd: impl AsRef<Path>,
        lsp_client: Option<SharedLspClient>,
    ) -> anyhow::Result<(ProviderAgents, UnboundedReceiver<AgentEvent>)> {
        let model = config.model().to_owned();
        let cwd = cwd.as_ref().to_path_buf();

        let client = aries_core::create_client(config.clone())?;

        let (agents, receiver) = match client {
            AriesClient::OpenAI(client) => {
                let (agent, receiver) = AgentBuilder::<openai::CompletionsClient>::new(
                    client.clone(),
                    &model,
                    agent_type,
                    cwd,
                )
                .with_tools(lsp_client)
                .await;
                (ProviderAgents::OpenAICompatible { agent }, receiver)
            },
            AriesClient::Azure(client) => {
                let (agent, receiver) =
                    AgentBuilder::<azure::Client>::new(client.clone(), &model, agent_type, cwd)
                        .with_tools(lsp_client)
                        .await;
                (ProviderAgents::Azure { agent }, receiver)
            },
            AriesClient::DeepSeek(client) => {
                let (agent, receiver) =
                    AgentBuilder::<deepseek::Client>::new(client.clone(), &model, agent_type, cwd)
                        .with_tools(lsp_client)
                        .await;
                (ProviderAgents::DeepSeek { agent }, receiver)
            },
        };

        Ok((agents, receiver))
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
