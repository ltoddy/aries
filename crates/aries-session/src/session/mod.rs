mod args;
mod config;
mod hook;
mod instruction;
mod question;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use aries_agent::AriesAgentProvider;
use aries_compact::{self, ContextCompactor, TokenEstimator};
use aries_context::{ChatContext, ChatHistory};
use aries_event::{AgentEvent, Notifier};
use aries_extension::hook::input::{
    SessionEndHookInput, SessionEndReason, SessionStartHookInput, SessionStartSource,
    StopFailureHookInput, StopHookInput, UserPromptSubmitHookInput,
};
use aries_extension::hook::{HookDecision, HooksExecutor};
use aries_extension::mcp::McpDefinition;
use aries_extension::{AgentExtensions, mcp};
use aries_init::{GlobalContext, ModelConfig, Setting, SettingLoader};
use aries_lspclient::{LspServerInfo, SharedLspClient, warm_up};
use aries_memory::MemoryStore;
use aries_mode::Mode;
use aries_persistence::SessionRepository;
use aries_tools::{edit, write};
use itertools::Itertools;
use jiff::Zoned;
use rig_agent::agent::PromptResponse;
use rig_agent::tool::rmcp::McpClientHandler;
use rig_agent::tool::server::{ToolServer, ToolServerHandle};
use rig_core::OneOrMany;
use rig_core::completion::{Message, Usage};
use rig_core::message::{AssistantContent, UserContent};
use rmcp::RoleClient;
use rmcp::service::RunningService;
use toasty::Db;
use tokio::pin;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;
use tracing::info;

pub use self::args::SessionArgs;
use self::config::SessionConfig;
use self::hook::SessionPromptHook;
pub use self::question::resume_input;
use crate::AriesClientProvider;
use crate::commands::CommandsExecutor;

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
    transcript_path: PathBuf,

    tool_server_handle: ToolServerHandle,
    cancel_token: CancellationToken,
    receiver: Arc<Mutex<UnboundedReceiver<AgentEvent>>>,
    notifier: Notifier,

    compactor: ContextCompactor,
    hooks_executor: Arc<HooksExecutor>,
    memory_store: MemoryStore,

    // 为了避免连接因为 drop 而释放
    #[allow(dead_code)]
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

    pub async fn set_model(&mut self, alias: impl Into<String>) -> anyhow::Result<()> {
        let alias = alias.into();

        let config = self.setting.activate(&alias)?;

        self.client = AriesClientProvider::new(&config)?;
        let agent = self
            .client
            .agent(
                self.mode,
                config.clone(),
                self.cwd.clone(),
                self.gctx.clone(),
                self.lsp_client.clone(),
                self.extensions.clone(),
                self.tool_server_handle.clone(),
                Notifier::clone(&self.notifier),
            )
            .await
            .with_context(|| format!("failed to create agent for model {alias}"))?;

        self.agent = agent;
        self.config = config.to_owned();
        self.compactor.set_agent(self.client.compact_agent(
            self.config.model(),
            &self.transcript_path,
            Notifier::clone(&self.notifier),
        ));

        let loader = SettingLoader::new(&self.gctx.root_dir);
        let _ = loader.save(&self.setting).await;

        Ok(())
    }

    pub async fn set_mode(&mut self, mode: Mode) -> anyhow::Result<()> {
        let agent = self
            .client
            .agent(
                mode,
                self.config.clone(),
                self.cwd.clone(),
                self.gctx.clone(),
                self.lsp_client.clone(),
                self.extensions.clone(),
                self.tool_server_handle.clone(),
                Notifier::clone(&self.notifier),
            )
            .await
            .with_context(|| format!("failed to create agent for mode {mode}"))?;

        self.agent = agent;
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

        if let Message::User { ref content } = prompt
            && let UserContent::Text(text) = content.first()
            && let Some(input) = text.text.trim().strip_prefix("/")
            && self.try_execute_slash_command(input, callback.clone()).await
        {
            return Ok(());
        }

        let title = self.update_title(&prompt).await;
        let now = Zoned::now();
        self.notifier.send_session_info_update(&title, now.to_string());

        self.fire_user_prompt_submit(&title).await?;
        self.pre_compact(&prompt, callback.clone()).await;

        let context = self.recall_context(&title).await;
        let mut history = self.chat_context.history().await.to_vec();
        if let Some(reminder) = context {
            history.push(Message::user(
                ["<system-reminder>", &reminder, "</system-reminder>"].join("\n"),
            ));
        }
        let final_res = {
            let hook = self.session_hook();
            let future = self.agent.prompt(prompt, history, hook);
            pin!(future);

            let mut guard = self.receiver.lock().await;
            let mut final_res = Ok(PromptResponse::empty());
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
                        final_res = res;
                        break;
                    }
                }
            }
            while let Ok(event) = guard.try_recv() {
                callback(event).await;
            }
            drop(guard);

            match final_res {
                Ok(res) => res,
                Err(err) => {
                    if err.is_awaiting_user_input() {
                        return Ok(());
                    }
                    self.fire_stop_failure(err.to_string()).await;
                    return Err(err);
                },
            }
        };

        self.last_assistant_message = Some(final_res.output().to_owned());
        if let Some(messages) = final_res.messages() {
            self.append_messages(messages).await;
        }
        self.fire_stop(final_res.output()).await;
        self.sift(final_res.messages(), title, final_res.output());
        if let Some(complection) = final_res.completion_calls.last() {
            self.post_compact(complection.usage, callback).await;
        }

        Ok(())
    }

    pub fn sift(
        &self,
        messages: Option<&[Message]>,
        query: impl Into<String>,
        reply: impl Into<String>,
    ) {
        if messages
            .map(|messages| agent_wrote_memory(messages, self.memory_store.dir()))
            .unwrap_or(false)
        {
            info!("本轮主模型已直接写入记忆，跳过后台记忆代理");
            return;
        }

        let client = self.client.clone();
        let model = self.config.model();
        let memory_store = self.memory_store.clone();
        let query = query.into();
        let reply = reply.into();
        tokio::spawn(async move {
            let manifest = memory_store.read_manifest().await.ok().flatten();
            let memory_agent = client.memory_agent(model, memory_store.dir()).await;
            memory_agent.run(manifest, query, reply).await;
        });
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
        &self.transcript_path
    }

    pub async fn close(&self) {
        let input = SessionEndHookInput::new(&self.id, &self.cwd, SessionEndReason::Logout)
            .transcript_path(&self.transcript_path);

        self.hooks_executor.fire_session_end(input).await;
    }

    pub fn list_slash_commands(&self) -> Vec<aries_extension::command::Frontmatter> {
        self.extensions.commands.iter().map(|c| c.frontmatter.clone()).collect_vec()
    }

    async fn try_execute_slash_command<F, Fut>(
        &mut self,
        input: impl AsRef<str>,
        callback: F,
    ) -> bool
    where
        F: Fn(AgentEvent) -> Fut + Clone,
        Fut: Future<Output = ()>,
    {
        let mut executor = CommandsExecutor::new(
            &self.agent,
            &self.extensions.commands,
            &self.id,
            ContextCompactor::clone(&self.compactor),
            Notifier::clone(&self.notifier),
        );
        if executor.execute(input).await {
            let mut guard = self.receiver.lock().await;
            while let Ok(event) = guard.try_recv() {
                callback(event).await;
            }
            return true;
        };
        false
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
        let transcript_path = session_dir.join("transcripts");

        aries_logger::register(&id, &session_dir);

        let (notifier, receiver) = Notifier::channel();

        let lsp_client = Self::warm_up_lsp(cwd).await;

        let mut extensions =
            if args.bare { AgentExtensions::empty() } else { AgentExtensions::new(cwd).await };
        extensions.mcps.push(external_mcp_config);
        let tool_server_handle = ToolServer::new().run();
        let mcp_clients = mcp::connect(&extensions.mcps, tool_server_handle.clone()).await;

        let memory_store =
            MemoryStore::new(gctx.memory_root_dir.join(aries_filesystem::path_to_slug(cwd))).await;

        let chat_history = ChatHistory::new(&session_dir)
            .await
            .with_context(|| format!("failed to initialize chat history for session {id}"))?;
        let chat_context = ChatContext::new(&session_dir)
            .await
            .with_context(|| format!("failed to initialize chat context for session {id}"))?;

        let session_repo = SessionRepository::new(db.clone());

        let mode = Mode::default();
        let client = AriesClientProvider::new(&config)?;
        let agent = client
            .agent(
                mode,
                config.clone(),
                cwd,
                gctx.clone(),
                lsp_client.clone(),
                extensions.clone(),
                tool_server_handle.clone(),
                Notifier::clone(&notifier),
            )
            .await?;

        let hooks_executor = Arc::new(HooksExecutor::new(extensions.hooks.clone()));
        let compactor = ContextCompactor::new(
            &id,
            cwd,
            &transcript_path,
            client.compact_agent(config.model(), &transcript_path, Notifier::clone(&notifier)),
            chat_context.clone(),
            hooks_executor.clone(),
            Notifier::clone(&notifier),
        );

        let input = SessionStartHookInput::new(&id, cwd, source, config.model(), mode);
        hooks_executor.fire_session_start(input).await;

        Ok(Self {
            id,
            gctx,
            setting,
            config,
            cwd: cwd.to_owned(),
            client,
            agent,
            mode,
            lsp_client,
            chat_history,
            chat_context,
            db,
            session_repo,
            session_dir: session_dir.to_owned(),
            transcript_path,
            tool_server_handle,
            cancel_token: CancellationToken::new(),
            notifier,
            receiver: Arc::new(Mutex::new(receiver)),
            hooks_executor,
            memory_store,
            mcp_clients: Arc::new(mcp_clients),
            extensions,
            last_assistant_message: None,
            compactor,
        })
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

    fn session_hook(&self) -> SessionPromptHook {
        SessionPromptHook::new(
            self.hooks_executor.clone(),
            &self.id,
            &self.cwd,
            &self.transcript_path,
            self.mode.id(),
            self.mode.name(),
            self.db.clone(),
            Notifier::clone(&self.notifier),
        )
    }

    async fn recall_context(&self, query: impl Into<String>) -> Option<String> {
        let memories = self.memory_store.scan().await;
        let retriever = self.client.memory_retriever(self.config.model());
        let retrieved = retriever.retrieve(query, &memories).await;

        let mut blocks = Vec::<String>::with_capacity(retrieved.len());
        for file_name in retrieved {
            if let Some(body) = self.memory_store.read_memory(&file_name).await {
                blocks.push(format!("## {file_name}\n\n{body}"));
            }
        }

        if blocks.is_empty() {
            return None;
        }
        Some(format!(
            "以下是与当前对话相关的历史记忆（自动召回，用户不可见）：\n\n{}",
            blocks.join("\n\n")
        ))
    }

    async fn fire_user_prompt_submit(
        &self,
        prompt: impl Into<String>,
    ) -> aries_agent::AriesResult<()> {
        let input = UserPromptSubmitHookInput::new(&self.id, &self.cwd, prompt)
            .transcript_path(&self.transcript_path);
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
            .transcript_path(&self.transcript_path)
            .error_details(&error);

        let input = match &self.last_assistant_message {
            Some(message) => input.last_assistant_message(message),
            None => input,
        };

        self.hooks_executor.fire_stop_failure(input).await;
    }

    async fn append_messages(&mut self, messages: &[Message]) {
        if messages.is_empty() {
            return;
        }

        self.chat_history.append(messages).await;
        self.chat_context.append(messages).await;
    }

    async fn pre_compact<F, Fut>(&mut self, prompt: &Message, mut callback: F)
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        {
            let mut write = self.chat_context.history_mut().await;
            aries_compact::micro_compact(&mut write, aries_compact::KEEP_RECENT);
        }

        let window = aries_compact::ContextWindow::new();
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
            self.compactor.compact().await;
        }
    }

    async fn post_compact<F, Fut>(&mut self, usage: Usage, mut callback: F)
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        let window = aries_compact::ContextWindow::new();
        let compact_threshold = window.auto_compact_threshold();

        if usage.total_tokens > compact_threshold {
            let text = format!(
                "\n实际 tokens {} 已达阈值 {compact_threshold}，触发压缩...\n",
                usage.total_tokens,
            );
            callback(AgentEvent::notification(text)).await;
            self.compactor.compact().await;
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

fn agent_wrote_memory(messages: &[Message], memory_dir: impl AsRef<Path>) -> bool {
    let memory_dir = memory_dir.as_ref();

    messages
        .iter()
        .filter_map(|m| match m {
            Message::Assistant { content, .. } => Some(content),
            _ => None,
        })
        .flat_map(OneOrMany::iter)
        .filter_map(|c| match c {
            AssistantContent::ToolCall(tc) => Some(tc),
            _ => None,
        })
        .filter(|tc| matches!(tc.function.name.as_str(), write::NAME | edit::NAME))
        .filter_map(|tc| tc.function.arguments.get("file_path").and_then(|v| v.as_str()))
        .any(|file_path| PathBuf::from(file_path).starts_with(memory_dir))
}
