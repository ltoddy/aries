mod args;
mod chat_context;
mod chat_history;
mod config;
mod hook;
mod instruction;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aries_compact::{
    self, AutoCompactBreaker, CompactAgent, CompactOutcome, Decision, TokenEstimator,
};
use aries_event::AgentEvent;
use aries_extension::hook::input::{
    PostCompactHookInput, PostCompactTrigger, PreCompactCustomInstructions, PreCompactHookInput,
    SessionEndHookInput, SessionEndReason, SessionStartHookInput, SessionStartSource,
    StopFailureHookInput, StopHookInput, UserPromptSubmitHookInput,
};
use aries_extension::hook::{HookDecision, HooksExecutor};
use aries_extension::mcp::McpDefinition;
use aries_extension::{AgentExtensions, mcp};
use aries_init::{GlobalContext, ModelConfig, Setting, SettingError};
use aries_lspclient::{LspServerInfo, SharedLspClient, warm_up};
use aries_memory::MemoryStore;
use aries_mode::Mode;
use aries_persistence::SessionRepository;
use futures::pin_mut;
use rig_core::agent::PromptResponse;
use rig_core::completion::Message;
use rig_core::message::UserContent;
use rig_core::wasm_compat::WasmCompatSend;
use rmcp::RoleClient;
use rmcp::model::ClientInfo;
use rmcp::service::RunningService;
use toasty::Db;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

pub use self::args::SessionArgs;
use self::chat_context::ChatContext;
use self::chat_history::ChatHistory;
use self::config::SessionConfig;
use self::hook::SessionPromptHook;
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
    chat_context: ChatContext,

    db: Db,
    session_repo: SessionRepository,

    session_dir: PathBuf,
    transcripts_dir: PathBuf,

    cancel_token: CancellationToken,
    receiver: Arc<Mutex<UnboundedReceiver<AgentEvent>>>,

    compact_breaker: AutoCompactBreaker,
    hooks_executor: Arc<HooksExecutor>,
    memory_store: MemoryStore,

    mcp_clients: Arc<Vec<RunningService<RoleClient, ClientInfo>>>,
    extensions: AgentExtensions,

    last_assistant_message: Option<String>,
}

impl Session {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        id: impl Into<String>,
        gctx: GlobalContext,
        cwd: impl AsRef<Path>,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
        args: SessionArgs,
    ) -> anyhow::Result<Self> {
        let id = id.into();
        let cwd = cwd.as_ref();
        let session_dir = gctx.root_dir.join(format!("session-{id}"));
        _ = tokio::fs::create_dir_all(&session_dir).await;

        let session_config = SessionConfig::new(args.bare);
        session_config.save(&session_dir).await;

        Self::build(
            id,
            cwd,
            gctx,
            config,
            setting,
            db,
            external_mcp_config,
            SessionStartSource::Startup,
            args,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn load(
        id: impl Into<String>,
        gctx: GlobalContext,
        cwd: impl AsRef<Path>,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
    ) -> anyhow::Result<Self> {
        let id = id.into();
        let cwd = cwd.as_ref();
        let session_dir = gctx.root_dir.join(format!("session-{id}"));
        _ = tokio::fs::create_dir_all(&session_dir).await;

        let session_config = SessionConfig::load(&session_dir).await;

        Self::build(
            id,
            cwd,
            gctx,
            config,
            setting,
            db,
            external_mcp_config,
            SessionStartSource::Resume,
            SessionArgs::new(session_config.bare),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn build(
        id: impl Into<String>,
        cwd: impl AsRef<Path>,
        gctx: GlobalContext,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        _external_mcp_config: McpDefinition,
        source: SessionStartSource,
        args: SessionArgs,
    ) -> anyhow::Result<Self> {
        let id = id.into();
        let cwd = cwd.as_ref();
        let session_dir = gctx.root_dir.join(format!("session-{id}"));
        let transcripts_dir = session_dir.join("transcripts");

        let _ = tokio::fs::create_dir_all(&transcripts_dir).await;

        aries_logger::register(&id, &session_dir);

        let lsp_client = Self::warm_up_lsp(cwd).await;

        let extensions =
            if args.bare { AgentExtensions::empty() } else { AgentExtensions::new(cwd).await };
        let (mcp_clients, mcp_tools) = mcp::connect(&extensions.mcps).await;

        let mem_store = MemoryStore::new(&gctx.memory_dir).await;

        let chat_history = ChatHistory::new(&session_dir).await;
        let chat_context = ChatContext::new(&session_dir).await;

        let session_repo = SessionRepository::new(db.clone());

        let mode = Mode::default();
        let client = AriesClient::new(&config)?;
        let (agent, receiver) = client
            .agent(
                mode,
                config.clone(),
                cwd,
                gctx,
                lsp_client.clone(),
                extensions.clone(),
                mcp_tools,
            )
            .await?;

        let hooks_executor = Arc::new(HooksExecutor::new(extensions.hooks.clone()));

        let input = SessionStartHookInput::new(&id, cwd, source, config.model(), mode);
        hooks_executor.fire_session_start(input).await;

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
            chat_context,
            db,
            session_repo,
            session_dir: session_dir.to_path_buf(),
            transcripts_dir,
            cancel_token: CancellationToken::new(),
            receiver: Arc::new(Mutex::new(receiver)),
            compact_breaker: AutoCompactBreaker::new(),
            hooks_executor,
            memory_store: mem_store,
            mcp_clients: Arc::new(mcp_clients),
            extensions,
            last_assistant_message: None,
        })
    }

    pub async fn set_model(&mut self, alias: impl Into<String>) -> anyhow::Result<()> {
        let alias = alias.into();

        let config = self
            .setting
            .models
            .iter()
            .find(|m| m.alias() == alias)
            .ok_or_else(|| SettingError::not_found(&alias))?;

        self.client = AriesClient::new(config)?;
        // let memory = Self::load_memory(&self.memory_store).await;
        // let (agent, agent_events) = self
        //     .client
        //     .agent(
        //         self.mode,
        //         self.config.clone(),
        //         self.cwd.clone(),
        //         self.lsp_client.clone(),
        //         memory,
        //         self.extensions.clone(),
        //         self.mcp_tools.to_vec(),
        //     )
        //     .await?;

        // TODO: Agent 自己实现

        // self.agent = agent;
        // self.receiver = Arc::new(Mutex::new(agent_events));
        self.config = config.to_owned();
        self.setting.active = alias;
        Ok(())
    }

    pub async fn set_mode(&mut self, mode: Mode) -> anyhow::Result<()> {
        self.agent.set_mode(mode);
        self.mode = mode;
        Ok(())
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
        self.cancel_token = CancellationToken::new();
        self.last_assistant_message = None;

        let prompt: Message = prompt.into();
        let title = message_to_simple_text(&prompt);
        let user_msg_for_memory = title.clone();
        if let Err(err) = self.session_repo.update_title_by_session_id(&self.id, &title).await {
            warn!("failed to update title({title}): {err}");
        }

        let input = UserPromptSubmitHookInput::new(&self.id, &self.cwd, title)
            .transcript_path(&self.transcripts_dir);
        if let HookDecision::Terminate { reason } =
            self.hooks_executor.fire_user_prompt_submit(input).await
        {
            return Err(anyhow::anyhow!(reason));
        }

        aries_compact::micro_compact(self.chat_context.history_mut());

        let window = aries_compact::ContextWindow::for_model(self.config.model());
        let compact_threshold = window.auto_compact_threshold();
        let estimate_tokens =
            self.chat_context.history().estimate_tokens().saturating_add(prompt.estimate_tokens());

        if estimate_tokens >= compact_threshold {
            info!(
                "\n🔄 预估 tokens {estimate_tokens} 已达阈值 {compact_threshold}（上下文窗口 {}），提前触发压缩...",
                window.total
            );
            self.compact().await;
        }

        let final_res = {
            let snapshot = self.chat_context.history().to_vec();
            match self.run_prompt(prompt, &snapshot, &mut cb).await {
                Ok(res) => res,
                Err(err) => {
                    let input = StopFailureHookInput::new(&self.id, &self.cwd, err.to_string())
                        .transcript_path(&self.transcripts_dir)
                        .error_details(err.to_string());

                    let input = match &self.last_assistant_message.take() {
                        Some(message) => input.last_assistant_message(message),
                        None => input,
                    };

                    self.hooks_executor.fire_stop_failure(input).await;
                    return Err(err);
                },
            }
        };

        self.last_assistant_message = Some(final_res.output().to_owned());
        if let Some(his) = final_res.messages() {
            self.chat_history.extend(his.iter().cloned());
            self.chat_context.extend(his.iter().cloned()).await;
        }

        let input = StopHookInput::new(&self.id, &self.cwd, false)
            .last_assistant_message(final_res.output());
        self.hooks_executor.fire_stop(input).await;

        // Spawn background memory extraction task
        {
            let client = self.client.clone();
            let model: String = self.config.model();
            let memory_store = self.memory_store.clone();
            let user_msg = user_msg_for_memory.clone();
            let assistant_resp = final_res.output().to_owned();
            tokio::spawn(async move {
                client.extract_memories(&model, &user_msg, &assistant_resp, &memory_store).await;
            });
        }

        if final_res.usage().total_tokens > compact_threshold {
            info!(
                "\n🔄 实际 tokens {} 已达阈值 {compact_threshold}，触发压缩...",
                final_res.usage().total_tokens,
            );
            self.compact().await;
        }

        Ok(())
    }

    pub async fn compact(&mut self) -> bool {
        match self.compact_breaker.decide() {
            Decision::Skip { wait, consecutive_failures } => {
                info!(
                    "\n⏳ 压缩处于冷却期（已连续失败 {consecutive_failures} 次），约 {wait:?} 后重试，本次跳过。",
                );
                return false;
            },
            Decision::Allow { half_open } => {
                if half_open {
                    info!("\n🔁 冷却结束，尝试恢复压缩...");
                }
            },
        }

        let input = PreCompactHookInput::new(
            &self.id,
            &self.cwd,
            PostCompactTrigger::Auto,
            PreCompactCustomInstructions::Auto,
        )
        .transcript_path(&self.transcripts_dir);
        if let HookDecision::Terminate { .. } = self.hooks_executor.fire_pre_compact(input).await {
            return false;
        }

        let outcome = match self.client.clone() {
            AriesClient::Anthropic(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcripts_dir);
                compact_agent.compact(self.chat_context.history()).await
            },
            AriesClient::Azure(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcripts_dir);
                compact_agent.compact(self.chat_context.history()).await
            },
            AriesClient::Deepseek(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcripts_dir);
                compact_agent.compact(self.chat_context.history()).await
            },
            AriesClient::OpenAI(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcripts_dir);
                compact_agent.compact(self.chat_context.history()).await
            },
        };

        match outcome {
            CompactOutcome::Success((compressed, compact_summary)) => {
                self.chat_context.overwrite(compressed).await;

                let window = aries_compact::ContextWindow::for_model(self.config.model());
                let post_tokens = self.chat_context.history().estimate_tokens();
                let threshold = window.auto_compact_threshold();
                if post_tokens >= threshold {
                    info!(
                        "\n⚠️  压缩后 tokens {post_tokens} 仍高于阈值 {threshold}，本次压缩无效。",
                    );
                    self.compact_breaker.on_failure();
                    return false;
                }

                self.compact_breaker.on_success();

                let input = PostCompactHookInput::new(
                    &self.id,
                    &self.cwd,
                    PostCompactTrigger::Auto,
                    compact_summary,
                )
                .transcript_path(&self.transcripts_dir);
                self.hooks_executor.fire_post_compact(input).await;
                true
            },
            CompactOutcome::Transient(err) => {
                info!("\n🌐 压缩遇到临时错误（不计入失败）：{}", err);
                false
            },
            CompactOutcome::PromptTooLong => {
                info!("\n🛑 上下文过长，压缩请求被拒，进入冷却以避免反复重试。");
                self.compact_breaker.trip();
                false
            },
            CompactOutcome::Empty => {
                self.compact_breaker.on_failure();
                let failures = self.compact_breaker.consecutive_failures();
                if failures >= AutoCompactBreaker::MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES {
                    info!(
                        "\n⛔ 连续 {failures} 次压缩失败，进入 {} 分钟冷却。",
                        AutoCompactBreaker::AUTOCOMPACT_FAILURE_COOLDOWN.as_secs() / 60,
                    );
                }
                false
            },
        }
    }

    async fn run_prompt<F, Fut>(
        &mut self,
        prompt: impl Into<Message> + WasmCompatSend,
        history: &[Message],
        cb: &mut Option<F>,
    ) -> anyhow::Result<PromptResponse>
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        let hook = SessionPromptHook::new(
            self.hooks_executor.clone(),
            &self.id,
            &self.cwd,
            &self.transcripts_dir,
            self.mode.id(),
            self.mode.name(),
            self.db.clone(),
        );
        let prompt_fut = self.agent.prompt(prompt, history.to_vec(), hook);
        pin_mut!(prompt_fut);

        let mut events_guard = self.receiver.lock().await;
        let mut final_res = PromptResponse::empty();

        loop {
            tokio::select! {
                biased;
                _ = self.cancel_token.cancelled() => {
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

    pub fn id(&self) -> String {
        self.id.clone()
    }

    pub fn system_prompt(&self) -> String {
        self.agent.system_prompt()
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn setting(&self) -> &Setting {
        &self.setting
    }

    pub async fn clear_context(&mut self) {
        self.chat_context.overwrite([]).await;
    }

    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    pub fn session_dir(&self) -> &Path {
        &self.session_dir
    }

    pub fn current_dir(&self) -> &Path {
        &self.cwd
    }

    pub fn transcript_path(&self) -> &Path {
        &self.transcripts_dir
    }

    pub async fn close(&self) {
        let input = SessionEndHookInput::new(&self.id, &self.cwd, SessionEndReason::Logout)
            .transcript_path(&self.transcripts_dir);

        self.hooks_executor.fire_session_end(input).await;
    }
}

fn message_to_simple_text(message: &Message) -> String {
    match message {
        Message::User { content } => match content.first() {
            UserContent::Text(text) => text.text.clone(),
            _ => String::new(),
        },
        _ => String::new(),
    }
}
