mod archive_session;
mod close_session;
mod commands;
mod context;
mod delete_session;
mod error;
mod history;
mod hook;
mod instruction;
mod prompt;
mod session_status;
mod set_session_mode;
mod set_session_title;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aries_communicate::{AgentEvent, Notifier};
use aries_extension::AgentExtensions;
use aries_extension::agent::AgentDefinition;
use aries_extension::hook::input::{SessionStartHookInput, SessionStartSource};
use aries_extension::hook::{HookDecision, HooksExecutor};
use aries_extension::mcp::{self, McpDefinition};
use aries_init::{GlobalContext, ModelConfig, Setting, SettingLoader};
use aries_persistence::SessionRepository;
use itertools::Itertools;
use rig::completion::{Effort, Message};
use rig::message::UserContent;
use rig::tool::rmcp::McpConnection;
use rig::tool::server::{ToolServer, ToolServerHandle};
use toasty::Db;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

pub use self::commands::BUILTIN_COMMANDS;
pub use self::context::ChatContext;
pub use self::error::SessionError;
pub use self::history::ChatHistory;
use self::hook::SessionPromptHook;
use self::instruction::InstructionContext;
pub use self::session_status::SessionStatus;
use crate::agent::AriesAgent;
use crate::compaction::ContextCompactor;
use crate::memory::MemoryStore;
use crate::state::State;

pub enum PromptOutcome {
    Completed(String),
    Cancelled(String),
}

pub const DEFAULT_AGENT_NAME: &str = "build";
pub const DEFAULT_EFFORT: Effort = Effort::High;

#[derive(Clone)]
pub struct Session {
    gctx: GlobalContext,
    setting: Setting,
    config: ModelConfig,
    agent: AriesAgent,

    state: State,

    chat_history: ChatHistory,
    chat_context: ChatContext,

    session_repo: SessionRepository,

    tool_server_handle: ToolServerHandle,
    cancel_token: Arc<parking_lot::Mutex<CancellationToken>>,
    receiver: Arc<Mutex<UnboundedReceiver<AgentEvent>>>,
    notifier: Notifier,
    db: Db,

    compactor: ContextCompactor,
    hooks_executor: Arc<HooksExecutor>,
    memory_store: MemoryStore,
    hook: SessionPromptHook,

    // 为了避免连接因为 drop 而释放
    #[allow(dead_code)]
    mcp_clients: Arc<Vec<McpConnection>>,
    extensions: AgentExtensions,

    last_assistant_message: Option<String>,
}

impl Session {
    pub async fn new(
        gctx: GlobalContext,
        cwd: impl Into<PathBuf>,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
    ) -> Result<Self, SessionError> {
        let id = nanoid::nanoid!();
        let cwd: PathBuf = cwd.into();

        Self::build(
            id,
            cwd,
            gctx,
            config,
            setting,
            db,
            external_mcp_config,
            SessionStartSource::Startup,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn load(
        id: impl Into<String>,
        gctx: GlobalContext,
        cwd: impl Into<PathBuf>,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
    ) -> Result<Self, SessionError> {
        let id = id.into();
        let cwd: PathBuf = cwd.into();

        Self::build(
            id,
            cwd,
            gctx,
            config,
            setting,
            db,
            external_mcp_config,
            SessionStartSource::Resume,
        )
        .await
    }

    pub async fn set_model(&mut self, alias: impl Into<String>) -> Result<(), SessionError> {
        let alias = alias.into();

        let mut setting = self.setting.clone();
        let config = setting.activate(alias).map_err(SessionError::setting)?;
        let model = crate::create_model(&config)?;

        let loader = SettingLoader::new(self.gctx.root_dir());
        loader.save(&setting).await.map_err(SessionError::setting)?;

        self.setting = setting;
        self.state.model(model);
        self.agent.set_model(self.state.model.clone());

        self.config = config;
        self.compactor.set_model(self.state.model.clone());

        Ok(())
    }

    pub async fn set_thought_level(&mut self, level: impl AsRef<str>) -> Result<(), SessionError> {
        let level = level.as_ref();
        let effort = match level {
            "minimal" => Effort::Minimal,
            "low" => Effort::Low,
            "medium" => Effort::Medium,
            "high" => Effort::High,
            "xhigh" => Effort::XHigh,
            "max" => Effort::Max,
            _ => return Err(SessionError::invalid_thought_level(level)),
        };

        self.state.effort(effort);

        let agent = crate::agent::Builder::new(
            self.state.clone(),
            self.extensions.clone(),
            self.tool_server_handle.clone(),
            self.gctx.clone(),
            Notifier::clone(&self.notifier),
            self.db.clone(),
        )
        .build(self.hook.clone())
        .await;
        self.agent = agent;

        Ok(())
    }

    pub fn id(&self) -> String {
        self.state.session_id.clone()
    }

    pub fn mode(&self) -> &str {
        &self.state.agent.frontmatter.name
    }

    pub fn thought_level(&self) -> Effort {
        self.state.effort
    }

    pub fn setting(&self) -> &Setting {
        &self.setting
    }

    pub fn agents(&self) -> &[AgentDefinition] {
        &self.extensions.agents
    }

    pub async fn context(&self) -> Vec<Message> {
        self.chat_context.history().await.to_vec()
    }

    pub async fn history(&self) -> io::Result<Vec<Message>> {
        self.chat_history.messages().await
    }

    pub async fn overwrite_context(&self, messages: impl IntoIterator<Item = Message>) {
        self.chat_context.overwrite(messages).await;
    }

    pub async fn clear_context(&mut self) {
        self.chat_context.overwrite([]).await;
    }

    pub fn cancel(&self) {
        self.cancel_token.lock().cancel()
    }

    pub fn session_dir(&self) -> &Path {
        &self.state.session_dir
    }

    pub fn current_dir(&self) -> &Path {
        &self.state.cwd
    }

    pub fn transcript_dir(&self) -> &Path {
        &self.state.transcript_dir
    }

    pub fn list_available_commands(&self) -> Vec<aries_extension::AvailableCommand> {
        self.extensions.available_commands()
    }

    #[allow(clippy::too_many_arguments)]
    async fn build(
        id: impl Into<String>,
        cwd: impl Into<PathBuf>,
        gctx: GlobalContext,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
        source: SessionStartSource,
    ) -> Result<Self, SessionError> {
        let id = id.into();
        let cwd: PathBuf = cwd.into();
        let session_dir = gctx.root_dir().join(format!("session-{id}"));
        let transcript_dir = session_dir.join("transcripts");

        let _ = tokio::fs::create_dir_all(&session_dir).await;
        let _ = tokio::fs::create_dir_all(&transcript_dir).await;

        aries_logger::register(&id, &session_dir);

        let (notifier, receiver) = Notifier::channel();

        let mut extensions = AgentExtensions::new(&cwd, gctx.root_dir()).await;
        extensions.mcps.push(external_mcp_config);

        let tool_server_handle = ToolServer::new().run();
        let mcp_clients = mcp::connect(&extensions.mcps, tool_server_handle.clone()).await;

        let memory_store =
            MemoryStore::new(gctx.memory_root_dir().join(aries_filesystem::path_to_slug(&cwd)))
                .await;

        let chat_history = ChatHistory::new(&session_dir).await.map_err(SessionError::io)?;
        let chat_context = ChatContext::new(&session_dir).await.map_err(SessionError::io)?;

        let hooks_executor = Arc::new(HooksExecutor::new(extensions.hooks.clone()));
        let instruction_ctx = InstructionContext::new(&cwd);
        let mut session_repo = SessionRepository::new(db.clone());

        let hook = SessionPromptHook::new(
            hooks_executor.clone(),
            &id,
            &cwd,
            &transcript_dir,
            DEFAULT_AGENT_NAME,
            instruction_ctx.clone(),
            Notifier::clone(&notifier),
        );

        let model = crate::create_model(&config)?;
        let agent = extensions
            .agents
            .iter()
            .find(|agent| agent.frontmatter.name.eq_ignore_ascii_case(DEFAULT_AGENT_NAME))
            .cloned()
            .ok_or_else(|| SessionError::agent_not_found(DEFAULT_AGENT_NAME))?;

        let state = State::new(model, agent, cwd, session_dir, &id, transcript_dir);
        let agent = crate::agent::Builder::new(
            state.clone(),
            extensions.clone(),
            tool_server_handle.clone(),
            gctx.clone(),
            Notifier::clone(&notifier),
            db.clone(),
        )
        .build(hook.clone())
        .await;

        let compactor = ContextCompactor::new(
            state.clone(),
            chat_context.clone(),
            hooks_executor.clone(),
            Notifier::clone(&notifier),
        );

        session_repo
            .create(
                &id,
                state.cwd.display().to_string(),
                state.session_dir.display().to_string(),
                state.transcript_dir.display().to_string(),
            )
            .await
            .map_err(SessionError::db)?;

        let input = SessionStartHookInput::new(&id, &state.cwd, source, state.model.name());
        if let HookDecision::Continue { contexts } = hooks_executor.fire_session_start(input).await
        {
            chat_context.append(&system_reminders(contexts)).await;
        }

        Ok(Self {
            gctx,
            setting,
            config,
            agent,
            state,
            chat_history,
            chat_context,
            session_repo,
            tool_server_handle,
            cancel_token: Arc::new(parking_lot::Mutex::new(CancellationToken::new())),
            notifier,
            receiver: Arc::new(Mutex::new(receiver)),
            hooks_executor,
            memory_store,
            hook,
            mcp_clients: Arc::new(mcp_clients),
            extensions,
            last_assistant_message: None,
            compactor,
            db,
        })
    }
}

fn system_reminders<Item: AsRef<str>>(contents: impl IntoIterator<Item = Item>) -> Vec<Message> {
    let mut messages = Vec::new();
    for content in contents {
        messages.push(system_reminder(content));
    }
    messages
}

fn system_reminder(content: impl AsRef<str>) -> Message {
    let content = content.as_ref();

    Message::user(["<system-reminder>", content, "</system-reminder>"].join("\n"))
}

fn message_to_simple_text(message: &Message) -> String {
    match message {
        Message::User { content } => content
            .iter()
            .filter_map(|chunk| match chunk {
                UserContent::Text(t) => Some(t.text()),
                _ => None,
            })
            .join(""),
        _ => String::new(),
    }
}
