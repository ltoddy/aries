mod error;
mod hook;
mod instruction;

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aries_communicate::{AgentEvent, Notifier};
use aries_extension::AgentExtensions;
use aries_extension::agent::AgentDefinition;
use aries_extension::command::SlashCommandsExecutor;
use aries_extension::hook::input::{
    SessionEndHookInput, SessionEndReason, SessionStartHookInput, SessionStartSource,
    StopFailureHookInput, StopHookInput, UserPromptSubmitHookInput,
};
use aries_extension::hook::{HookDecision, HooksExecutor};
use aries_extension::mcp::{self, McpDefinition};
use aries_extension::skill::SkillExecutor;
use aries_init::{GlobalContext, ModelConfig, Setting, SettingLoader};
use aries_persistence::SessionRepository;
use itertools::Itertools;
use jiff::Zoned;
use rig::agent::PromptResponse;
use rig::completion::{Effort, Message};
use rig::message::UserContent;
use rig::tool::rmcp::McpConnection;
use rig::tool::server::{ToolServer, ToolServerHandle};
use toasty::Db;
use tokio::pin;
use tokio::sync::Mutex;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_util::sync::CancellationToken;

pub use self::error::SessionError;
use self::hook::SessionPromptHook;
use self::instruction::InstructionContext;
use crate::agent::{AgentBuilder, AriesAgent};
use crate::compact::ContextCompactor;
use crate::memory::{MemoryRetriever, MemoryStore};
use crate::session::{ChatContext, ChatHistory};
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

    transcript_path: PathBuf,

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
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
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

        let config = self.setting.activate(alias)?;

        let model = crate::session::create_model(&config);

        self.state.model(model);
        self.agent.set_model(self.state.model.clone());

        self.config = config.to_owned();
        self.compactor.set_model(self.state.model.clone());

        let loader = SettingLoader::new(self.gctx.root_dir());
        let _ = loader.save(&self.setting).await;

        Ok(())
    }

    pub async fn set_mode(&mut self, mode: impl AsRef<str>) -> Result<(), SessionError> {
        let mode = mode.as_ref();
        let agent = self
            .extensions
            .agents
            .iter()
            .find(|agent| agent.frontmatter.name.eq_ignore_ascii_case(mode))
            .cloned()
            .ok_or_else(|| SessionError::agent_not_found(mode))?;

        self.state.agent(agent);

        // 不同的模式, 注册的工具是不同的，所以先删除全部内置的，然后再重新添加
        for tool_name in crate::ALL_TOOL_NAMES {
            self.tool_server_handle.remove_tool(tool_name);
        }
        let agent = AgentBuilder::new(
            self.state.clone(),
            self.gctx.clone(),
            Notifier::clone(&self.notifier),
            self.db.clone(),
        )
        .with_extensions(self.extensions.clone())
        .build(self.tool_server_handle.clone(), self.hook.clone())
        .await;
        self.agent = agent;

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

        let agent = AgentBuilder::new(
            self.state.clone(),
            self.gctx.clone(),
            Notifier::clone(&self.notifier),
            self.db.clone(),
        )
        .with_extensions(self.extensions.clone())
        .build(self.tool_server_handle.clone(), self.hook.clone())
        .await;
        self.agent = agent;

        Ok(())
    }

    #[tracing::instrument(name = "prompt", skip_all, fields(session_id = %self.state.session_id))]
    pub async fn prompt<F, Fut>(
        &mut self,
        prompt: impl Into<Message>,
        callback: F,
    ) -> crate::agent::AriesResult<PromptOutcome>
    where
        F: Fn(AgentEvent) -> Fut + Clone,
        Fut: Future<Output = ()>,
    {
        let mut prompt: Message = prompt.into();
        let cancel_token = {
            let mut guard = self.cancel_token.lock();
            *guard = CancellationToken::new();
            guard.clone()
        };
        self.last_assistant_message = None;
        let message_id = format!("user-{}", nanoid::nanoid!());

        if let Message::User { ref content } = prompt
            && let Some(UserContent::Text(text)) = content.first()
            && let Some(input) = text.text.trim().strip_prefix("/")
            && let Some(slash_command) = self.try_execute_slash_command(input).await
        {
            prompt = Message::user(slash_command);
        }

        let title = self.update_title(&prompt).await;
        let now = Zoned::now();
        self.notifier.send_session_info_update(&title, now.to_string());

        let hook_context = self.fire_user_prompt_submit(&title).await?;
        self.compactor.pre_compact(&prompt).await;

        let context = self.recall_context(&title).await;
        let mut history = self.chat_context.history().await.to_vec();
        history.extend_from_slice(&system_reminders(hook_context));
        if let Some(reminder) = context {
            history.push(system_reminder(reminder));
        }
        let final_res = {
            let future = self.agent.prompt(prompt, history);
            pin!(future);

            let mut guard = self.receiver.lock().await;
            let mut final_res = Ok(PromptResponse::empty());
            let mut cancelled = false;
            loop {
                tokio::select! {
                    biased;
                    _ = cancel_token.cancelled() => {
                        cancelled = true;
                        break;
                    }
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

            if cancelled {
                return Ok(PromptOutcome::Cancelled(message_id));
            }

            match final_res {
                Ok(res) => res,
                Err(err) => {
                    self.fire_stop_failure(err.to_string()).await;
                    return Err(err);
                },
            }
        };

        let output = final_res.output();
        self.last_assistant_message = Some(output.clone());
        let messages = final_res.messages();
        self.append_messages(messages).await;
        self.fire_stop(&output).await;
        if let Some(completion) = final_res.completion_calls.last() {
            self.compactor.post_compact(completion.usage).await;
        }

        Ok(PromptOutcome::Completed(message_id))
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

    pub async fn overwrite_context(&self, messages: Vec<Message>) {
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

    pub fn transcript_path(&self) -> &Path {
        &self.transcript_path
    }

    pub async fn close(&self) {
        let input = SessionEndHookInput::new(
            &self.state.session_id,
            &self.state.cwd,
            SessionEndReason::Logout,
        )
        .transcript_path(&self.transcript_path);

        self.hooks_executor.fire_session_end(input).await;
    }

    pub fn list_available_commands(&self) -> Vec<aries_extension::AvailableCommand> {
        self.extensions.available_commands()
    }

    async fn try_execute_slash_command(&mut self, input: impl AsRef<str>) -> Option<String> {
        let input = input.as_ref();
        let executor = SlashCommandsExecutor::new(&self.extensions.commands);
        if let Some(command) = executor.execute(input).await {
            return Some(command);
        }

        let executor = SkillExecutor::new(&self.extensions.skills);
        executor.execute(input).await
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
        let transcript_path = session_dir.join("transcripts");

        let _ = tokio::fs::create_dir_all(&session_dir).await;
        let _ = tokio::fs::create_dir_all(&transcript_path).await;

        aries_logger::register(&id, &session_dir);

        let (notifier, receiver) = Notifier::channel();

        let mut extensions = AgentExtensions::new(&cwd, gctx.root_dir()).await;
        extensions.mcps.push(external_mcp_config);
        let tool_server_handle = ToolServer::new().run();
        let mcp_clients = mcp::connect(&extensions.mcps, tool_server_handle.clone()).await;

        let memory_store =
            MemoryStore::new(gctx.memory_root_dir().join(aries_filesystem::path_to_slug(&cwd)))
                .await;

        let chat_history = ChatHistory::new(&session_dir).await?;
        let chat_context = ChatContext::new(&session_dir).await?;

        let hooks_executor = Arc::new(HooksExecutor::new(extensions.hooks.clone()));
        let instruction_ctx = InstructionContext::new(&cwd);
        let session_repo = SessionRepository::new(db.clone());

        let hook = SessionPromptHook::new(
            hooks_executor.clone(),
            &id,
            &cwd,
            &transcript_path,
            DEFAULT_AGENT_NAME,
            db.clone(),
            instruction_ctx.clone(),
            Notifier::clone(&notifier),
        );

        let model = crate::session::create_model(&config);
        let agent = extensions
            .agents
            .iter()
            .find(|agent| agent.frontmatter.name.eq_ignore_ascii_case(DEFAULT_AGENT_NAME))
            .cloned()
            .ok_or_else(|| SessionError::agent_not_found(DEFAULT_AGENT_NAME))?;
        let state = State::new(model, agent, cwd, session_dir, &id);

        let agent =
            AgentBuilder::new(state.clone(), gctx.clone(), Notifier::clone(&notifier), db.clone())
                .with_extensions(extensions.clone())
                .build(tool_server_handle.clone(), hook.clone())
                .await;

        let compactor = ContextCompactor::new(
            &id,
            &state.cwd,
            &transcript_path,
            state.model.clone(),
            chat_context.clone(),
            hooks_executor.clone(),
            Notifier::clone(&notifier),
        );

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
            transcript_path,
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

    async fn update_title(&mut self, prompt: &Message) -> String {
        let title = message_to_simple_text(prompt);
        let _ = self.session_repo.update_title_by_session_id(&self.state.session_id, &title).await;
        title
    }

    async fn recall_context(&self, query: impl AsRef<str>) -> Option<String> {
        let query = query.as_ref();
        let memories = self.memory_store.scan().await;
        let retriever = MemoryRetriever::new(self.state.model.clone());
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
    ) -> crate::agent::AriesResult<VecDeque<String>> {
        let input = UserPromptSubmitHookInput::new(&self.state.session_id, &self.state.cwd, prompt)
            .transcript_path(&self.transcript_path);
        match self.hooks_executor.fire_user_prompt_submit(input).await {
            HookDecision::Terminate { reason } => {
                Err(crate::agent::AriesError::hook_terminated(reason))
            },
            HookDecision::Continue { contexts } => Ok(contexts),
        }
    }

    async fn fire_stop(&self, assistant_output: impl Into<String>) {
        let input = StopHookInput::new(&self.state.session_id, &self.state.cwd, false)
            .last_assistant_message(assistant_output);
        if let HookDecision::Continue { contexts } = self.hooks_executor.fire_stop(input).await {
            self.chat_context.append(&system_reminders(contexts)).await;
        }
    }

    async fn fire_stop_failure(&self, error: impl Into<String>) {
        let error = error.into();
        let input = StopFailureHookInput::new(&self.state.session_id, &self.state.cwd, &error)
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
