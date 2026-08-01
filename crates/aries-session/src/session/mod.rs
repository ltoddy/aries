mod args;
mod chat_context;
mod chat_history;
mod config;
mod hook;
mod instruction;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
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
use rig_agent::agent::PromptResponse;
use rig_agent::tool::rmcp::McpClientHandler;
use rig_agent::tool::server::{ToolServer, ToolServerHandle};
use rig_core::completion::{Message, Usage};
use rig_core::message::UserContent;
use rmcp::RoleClient;
use rmcp::service::RunningService;
use toasty::Db;
use tokio::pin;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub use self::args::SessionArgs;
use self::chat_context::ChatContext;
use self::chat_history::ChatHistory;
use self::config::SessionConfig;
use self::hook::SessionPromptHook;
use crate::{AriesAgentProvider, AriesClientProvider};

#[derive(Clone)]
pub struct Session {
    id: String,

    gctx: GlobalContext,
    setting: Setting,
    config: ModelConfig,
    cwd: PathBuf,
    client: AriesClientProvider,
    agent: AriesAgentProvider,
    mode: Mode,

    lsp_client: Option<SharedLspClient>,
    chat_history: ChatHistory,
    chat_context: ChatContext,

    db: Db,
    session_repo: SessionRepository,

    session_dir: PathBuf,
    transcripts_dir: PathBuf,

    tool_server_handle: ToolServerHandle,
    cancel_token: CancellationToken,
    receiver: Arc<Mutex<UnboundedReceiver<AgentEvent>>>,

    compact_breaker: AutoCompactBreaker,
    hooks_executor: Arc<HooksExecutor>,
    memory_store: MemoryStore,

    mcp_clients: Arc<Vec<RunningService<RoleClient, McpClientHandler>>>,
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
        external_mcp_config: McpDefinition,
        source: SessionStartSource,
        args: SessionArgs,
    ) -> anyhow::Result<Self> {
        let id = id.into();
        let cwd = cwd.as_ref();
        let session_dir = gctx.root_dir.join(format!("session-{id}"));
        let transcripts_dir = session_dir.join("transcripts");

        aries_logger::register(&id, &session_dir);

        let lsp_client = Self::warm_up_lsp(cwd).await;

        let mut extensions =
            if args.bare { AgentExtensions::empty() } else { AgentExtensions::new(cwd).await };
        extensions.mcps.push(external_mcp_config);
        let tool_server_handle = ToolServer::new().run();
        let mcp_services = mcp::connect(&extensions.mcps, tool_server_handle.clone()).await;

        let mem_store = MemoryStore::new(&gctx.memory_dir).await;

        let chat_history = ChatHistory::new(&session_dir).await;
        let chat_context = ChatContext::new(&session_dir).await;

        let session_repo = SessionRepository::new(db.clone());

        let mode = Mode::default();
        let client = AriesClientProvider::new(&config)?;
        let (agent, receiver) = client
            .agent(
                mode,
                config.clone(),
                cwd,
                gctx.clone(),
                lsp_client.clone(),
                extensions.clone(),
                tool_server_handle.clone(),
            )
            .await?;

        let hooks_executor = Arc::new(HooksExecutor::new(extensions.hooks.clone()));

        let input = SessionStartHookInput::new(&id, cwd, source, config.model(), mode);
        hooks_executor.fire_session_start(input).await;

        Ok(Self {
            id,
            gctx,
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
            tool_server_handle,
            cancel_token: CancellationToken::new(),
            receiver: Arc::new(Mutex::new(receiver)),
            compact_breaker: AutoCompactBreaker::new(),
            hooks_executor,
            memory_store: mem_store,
            mcp_clients: Arc::new(mcp_services),
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

        self.client = AriesClientProvider::new(config)?;
        let (agent, receiver) = self
            .client
            .agent(
                self.mode,
                config.clone(),
                self.cwd.clone(),
                self.gctx.clone(),
                self.lsp_client.clone(),
                self.extensions.clone(),
                self.tool_server_handle.clone(),
            )
            .await
            .with_context(|| format!("failed to create agent for model {alias}"))?;

        self.agent = agent;
        self.receiver = Arc::new(Mutex::new(receiver));
        self.config = config.to_owned();
        self.setting.active = alias;
        Ok(())
    }

    pub async fn set_mode(&mut self, mode: Mode) -> anyhow::Result<()> {
        let (agent, receiver) = self
            .client
            .agent(
                mode,
                self.config.clone(),
                self.cwd.clone(),
                self.gctx.clone(),
                self.lsp_client.clone(),
                self.extensions.clone(),
                self.tool_server_handle.clone(),
            )
            .await
            .with_context(|| format!("failed to create agent for mode {mode}"))?;

        self.agent = agent;
        self.receiver = Arc::new(Mutex::new(receiver));
        self.mode = mode;
        Ok(())
    }

    #[tracing::instrument(name = "prompt", skip_all, fields(session_id = %self.id))]
    pub async fn prompt<F, Fut>(
        &mut self,
        prompt: impl Into<Message>,
        callback: F,
    ) -> aries_agent::AriesResult<()>
    where
        F: Fn(AgentEvent) -> Fut + Clone,
        Fut: Future<Output = ()>,
    {
        let prompt: Message = prompt.into();
        self.cancel_token = CancellationToken::new();
        self.last_assistant_message = None;
        let title = self.update_title(&prompt).await;

        self.fire_user_prompt_submit(&title).await?;
        self.pre_compact(&prompt, callback.clone()).await;

        let prompt_res = {
            let history = self.chat_context.history().to_vec();
            let hook = self.session_hook();
            let future = self.agent.prompt(prompt, history, hook);
            pin!(future);

            let mut guard = self.receiver.lock().await;
            let mut prompt_res = Ok(PromptResponse::empty());
            loop {
                tokio::select! {
                    biased;
                    _ = self.cancel_token.cancelled() => break,
                    event = guard.recv() => {
                        if let Some(event) = event {
                            callback(event).await;
                        }
                    }
                    res = &mut future => {
                        prompt_res = res;
                        break;
                    }
                }
            }
            while let Ok(event) = guard.try_recv() {
                callback.clone()(event).await;
            }
            drop(guard);

            match prompt_res {
                Ok(res) => res,
                Err(err) => {
                    self.fire_stop_failure(err.to_string()).await;
                    return Err(err);
                },
            }
        };

        self.last_assistant_message = Some(prompt_res.output().to_owned());
        if let Some(messages) = prompt_res.messages() {
            self.append_messages(messages);
        }
        self.fire_stop(prompt_res.output()).await;
        self.sift(title, prompt_res.output());
        self.post_compact(prompt_res.usage(), callback).await;

        Ok(())
    }

    pub fn sift(&self, query: impl Into<String>, reply: impl Into<String>) {
        let client = self.client.clone();
        let model = self.config.model();
        let memory_store = self.memory_store.clone();
        let query = query.into();
        let reply = reply.into();
        tokio::spawn(async move {
            client.extract_memories(&model, query, reply, &memory_store).await;
        });
    }

    #[tracing::instrument(name = "compact", skip_all, fields(session_id = %self.id))]
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
            AriesClientProvider::Anthropic(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcripts_dir);
                compact_agent.compact(self.chat_context.history()).await
            },
            AriesClientProvider::Azure(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcripts_dir);
                compact_agent.compact(self.chat_context.history()).await
            },
            AriesClientProvider::Deepseek(client) => {
                let mut compact_agent =
                    CompactAgent::new(client, self.config.model(), &self.transcripts_dir);
                compact_agent.compact(self.chat_context.history()).await
            },
            AriesClientProvider::OpenAI(client) => {
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

    async fn update_title(&mut self, prompt: &Message) -> String {
        let title = message_to_simple_text(prompt);
        let _ = self.session_repo.update_title_by_session_id(&self.id, &title).await;
        title
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

    pub fn system_prompt(&self) -> &str {
        self.agent.preamble()
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

    fn session_hook(&self) -> SessionPromptHook {
        SessionPromptHook::new(
            self.hooks_executor.clone(),
            &self.id,
            &self.cwd,
            &self.transcripts_dir,
            self.mode.id(),
            self.mode.name(),
            self.db.clone(),
        )
    }

    async fn fire_user_prompt_submit(
        &self,
        prompt: impl Into<String>,
    ) -> aries_agent::AriesResult<()> {
        let input = UserPromptSubmitHookInput::new(&self.id, &self.cwd, prompt)
            .transcript_path(&self.transcripts_dir);
        if let HookDecision::Terminate { reason } =
            self.hooks_executor.fire_user_prompt_submit(input).await
        {
            return Err(aries_agent::AriesError::hook_terminated(reason));
        }

        Ok(())
    }

    async fn fire_stop(&self, assistant_output: impl Into<String>) {
        let input =
            StopHookInput::new(&self.id, &self.cwd, false).last_assistant_message(assistant_output);
        self.hooks_executor.fire_stop(input).await;
    }

    async fn fire_stop_failure(&self, error: impl Into<String>) {
        let error = error.into();
        let input = StopFailureHookInput::new(&self.id, &self.cwd, &error)
            .transcript_path(&self.transcripts_dir)
            .error_details(&error);

        let input = match &self.last_assistant_message {
            Some(message) => input.last_assistant_message(message),
            None => input,
        };

        self.hooks_executor.fire_stop_failure(input).await;
    }

    fn append_messages(&mut self, messages: &[Message]) {
        if messages.is_empty() {
            return;
        }

        self.chat_history.extend(messages.iter().cloned());
        self.chat_context.extend(messages.iter().cloned());
    }

    async fn pre_compact<F, Fut>(&mut self, prompt: &Message, mut callback: F)
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        aries_compact::micro_compact(self.chat_context.history_mut());

        let window = aries_compact::ContextWindow::for_model(self.config.model());
        let compact_threshold = window.auto_compact_threshold();
        let estimate_tokens =
            self.chat_context.history().estimate_tokens().saturating_add(prompt.estimate_tokens());

        if estimate_tokens >= compact_threshold {
            let text = format!(
                "\n预估 tokens {estimate_tokens} 已达阈值 {compact_threshold}（上下文窗口 {}），提前触发压缩...\n",
                window.total
            );
            callback(AgentEvent::text(true, self.mode.name(), text)).await;
            self.compact().await;
        }
    }

    async fn post_compact<F, Fut>(&mut self, usage: Usage, mut callback: F)
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        let window = aries_compact::ContextWindow::for_model(self.config.model());
        let compact_threshold = window.auto_compact_threshold();

        if usage.total_tokens > compact_threshold {
            let text = format!(
                "\n实际 tokens {} 已达阈值 {compact_threshold}，触发压缩...\n",
                usage.total_tokens,
            );
            callback(AgentEvent::text(true, self.mode.name(), text)).await;
            self.compact().await;
        }
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
