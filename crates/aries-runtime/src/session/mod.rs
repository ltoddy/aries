mod agents;
mod archive_session;
mod close_session;
mod commands;
mod config;
mod delete_session;
mod error;
mod handle;
mod hook;
mod load_session;
mod new_session;
mod prompt;
mod resume_session;
mod session_status;
mod set_session_config_option;
mod set_session_mode;
mod set_session_title;

use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use aries_communicate::{AgentEvent, Notifier};
use aries_extension::AgentExtensions;
use aries_extension::agent::AgentDefinition;
use aries_extension::hook::input::{SessionStartHookInput, SessionStartSource};
use aries_extension::hook::{HookDecision, HooksExecutor};
use aries_extension::mcp::{self, McpDefinition};
use aries_init::{GlobalContext, ModelConfig, Setting};
use aries_persistence::{SessionRepository, SubagentRepository};
use itertools::Itertools;
use rig::completion::{Effort, Message};
use rig::message::UserContent;
use rig::tool::rmcp::McpConnection;
use rig::tool::server::{ToolServer, ToolServerHandle};
use toasty::Db;
use tokio::sync::mpsc::UnboundedReceiver;

pub use self::commands::BUILTIN_COMMANDS;
pub use self::config::{ParseSessionConfigError, SessionConfig};
pub use self::error::SessionError;
pub use self::handle::{SessionHandle, SessionSnapshot};
use self::hook::SessionPromptHook;
pub use self::session_status::SessionStatus;
use crate::agent::AriesAgent;
use crate::context::compaction::ContextCompactor;
use crate::context::{ChatContext, ChatHistory};
use crate::memory::MemoryStore;
use crate::sandbox::SandboxMode;
use crate::state::State;

pub enum PromptOutcome {
    Completed(String),
    Cancelled(String),
}

pub const DEFAULT_AGENT_NAME: &str = "build";

struct Session {
    gctx: GlobalContext,
    setting: Setting,
    config: ModelConfig,
    agent: AriesAgent,

    state: State,

    chat_history: ChatHistory,
    chat_context: ChatContext,

    session_repo: SessionRepository,
    subagent_repo: SubagentRepository,

    tool_server_handle: ToolServerHandle,
    receiver: UnboundedReceiver<AgentEvent>,
    notifier: Notifier,
    db: Db,

    compactor: ContextCompactor,
    hooks_executor: Arc<HooksExecutor>,
    memory_store: MemoryStore,
    hook: SessionPromptHook,

    #[allow(dead_code)]
    mcp_clients: Vec<McpConnection>,
    extensions: AgentExtensions,

    last_assistant_message: Option<String>,
}

impl Session {
    pub fn sandbox_mode(&self) -> SandboxMode {
        self.state.sandbox_mode()
    }

    pub fn mode(&self) -> String {
        self.state.agent().frontmatter.name
    }

    pub fn thought_level(&self) -> Effort {
        self.state.effort()
    }

    pub fn setting(&self) -> &Setting {
        &self.setting
    }

    pub fn agents(&self) -> &[AgentDefinition] {
        &self.extensions.agents
    }

    pub async fn context(&self) -> Vec<Message> {
        self.chat_context.history().await
    }

    pub async fn history(&self) -> io::Result<Vec<Message>> {
        self.chat_history.messages().await
    }

    pub async fn overwrite_context(&self, messages: impl IntoIterator<Item = Message>) {
        self.chat_context.overwrite(messages).await;
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
        let session_repo = SessionRepository::new(db.clone());
        let subagent_repo = SubagentRepository::new(db.clone());

        let model = crate::create_model(&config)?;
        let agent = extensions
            .agents
            .iter()
            .find(|agent| agent.frontmatter.name.eq_ignore_ascii_case(DEFAULT_AGENT_NAME))
            .cloned()
            .ok_or_else(|| SessionError::agent_not_found(DEFAULT_AGENT_NAME))?;

        let state = State::new(model, agent, &cwd, session_dir, &id, &transcript_dir);
        let hook = SessionPromptHook::new(
            state.clone(),
            hooks_executor.clone(),
            &id,
            &cwd,
            &transcript_dir,
            DEFAULT_AGENT_NAME,
            Notifier::clone(&notifier),
        );
        let agent = crate::agent::Builder::new(
            state.clone(),
            extensions.clone(),
            tool_server_handle.clone(),
            gctx.clone(),
            Notifier::clone(&notifier),
            db.clone(),
        )
        .build(chat_history.clone(), chat_context.clone(), hook.clone())
        .await;

        let compactor =
            ContextCompactor::new(state.clone(), chat_context.clone(), Notifier::clone(&notifier));

        let model = state.model();
        let input = SessionStartHookInput::new(&id, state.cwd(), source, model.name());
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
            subagent_repo,
            tool_server_handle,
            notifier,
            receiver,
            hooks_executor,
            memory_store,
            hook,
            mcp_clients,
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
