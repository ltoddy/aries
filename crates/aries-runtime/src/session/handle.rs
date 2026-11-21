use std::path::PathBuf;
use std::sync::Arc;
use std::{fmt, io};

use aries_communicate::AgentEvent;
use aries_extension::AvailableCommand;
use aries_extension::agent::AgentDefinition;
use aries_extension::mcp::McpDefinition;
use aries_init::{GlobalContext, ModelConfig, Setting};
use rig::completion::{Effort, Message};
use toasty::Db;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::agent::{AriesError, AriesResult};
use crate::sandbox::SandboxMode;
use crate::session::{PromptOutcome, Session, SessionError, SessionStatus};
use crate::state::State;

#[derive(Clone)]
pub struct SessionHandle {
    inner: Arc<SessionInner>,
}

struct SessionInner {
    state: State,
    session: Mutex<Session>,
    cancel_token: parking_lot::Mutex<CancellationToken>,
}

impl SessionInner {
    fn new(session: Session) -> Self {
        Self {
            state: session.state.clone(),
            session: Mutex::new(session),
            cancel_token: parking_lot::Mutex::new(CancellationToken::new()),
        }
    }
}

impl fmt::Debug for SessionHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionHandle").field("id", &self.id()).finish_non_exhaustive()
    }
}

impl SessionHandle {
    fn new(session: Session) -> Self {
        Self { inner: Arc::new(SessionInner::new(session)) }
    }

    pub async fn new_session(
        gctx: GlobalContext,
        cwd: impl Into<PathBuf>,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
    ) -> Result<Self, SessionError> {
        let session =
            Session::new_session(gctx, cwd, config, setting, db, external_mcp_config).await?;
        Ok(Self::new(session))
    }

    pub async fn load_session(
        id: impl Into<String>,
        gctx: GlobalContext,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
    ) -> Result<Self, SessionError> {
        let session =
            Session::load_session(id, gctx, config, setting, db, external_mcp_config).await?;
        Ok(Self::new(session))
    }

    pub async fn resume_session(
        id: impl Into<String>,
        gctx: GlobalContext,
        config: ModelConfig,
        setting: Setting,
        db: Db,
        external_mcp_config: McpDefinition,
    ) -> Result<Self, SessionError> {
        let session =
            Session::resume_session(id, gctx, config, setting, db, external_mcp_config).await?;
        Ok(Self::new(session))
    }

    pub fn id(&self) -> String {
        self.inner.state.session_id()
    }

    pub fn current_dir(&self) -> PathBuf {
        self.inner.state.cwd()
    }

    pub fn session_dir(&self) -> PathBuf {
        self.inner.state.session_dir()
    }

    pub fn transcript_dir(&self) -> PathBuf {
        self.inner.state.transcript_dir()
    }

    pub fn cancel(&self) {
        self.inner.cancel_token.lock().cancel();
    }

    pub async fn prompt<F, Fut>(
        &self,
        prompt: impl Into<Message>,
        callback: F,
    ) -> AriesResult<PromptOutcome>
    where
        F: FnMut(AgentEvent) -> Fut,
        Fut: Future<Output = ()>,
    {
        let (mut session, cancel_token) = {
            let mut token = self.inner.cancel_token.lock();
            let session =
                self.inner.session.try_lock().map_err(|_| AriesError::session_busy(self.id()))?;
            *token = CancellationToken::new();
            (session, token.clone())
        };
        session.prompt(prompt, callback, cancel_token).await
    }

    pub async fn snapshot(&self) -> SessionSnapshot {
        let session = self.inner.session.lock().await;
        SessionSnapshot::new(&session)
    }

    pub async fn context(&self) -> Result<Vec<Message>, SessionError> {
        let session = self.inner.session.try_lock().map_err(|_| SessionError::busy(self.id()))?;
        Ok(session.context().await)
    }

    pub async fn history(&self) -> io::Result<Vec<Message>> {
        self.inner.session.lock().await.history().await
    }

    pub async fn overwrite_context(&self, messages: impl IntoIterator<Item = Message>) {
        self.inner.session.lock().await.overwrite_context(messages).await;
    }

    pub async fn list_available_commands(&self) -> Vec<AvailableCommand> {
        self.inner.session.lock().await.list_available_commands()
    }

    pub async fn session_status(&self) -> SessionStatus {
        self.inner.session.lock().await.session_status()
    }

    pub async fn set_session_mode(&self, mode: impl AsRef<str>) -> Result<(), SessionError> {
        let mut session =
            self.inner.session.try_lock().map_err(|_| SessionError::busy(self.id()))?;
        session.set_session_mode(mode).await
    }

    pub async fn set_session_config_option(
        &self,
        config_id: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<(), SessionError> {
        let mut session =
            self.inner.session.try_lock().map_err(|_| SessionError::busy(self.id()))?;
        session.set_session_config_option(config_id, value).await
    }

    pub async fn set_session_title(&self, title: impl AsRef<str>) -> Result<(), SessionError> {
        self.inner.session.lock().await.set_session_title(title).await
    }

    pub async fn close_session(&self) {
        self.cancel();
        self.inner.session.lock().await.close_session().await;
    }

    pub async fn archive_session(&self) -> Result<(), SessionError> {
        self.inner.session.lock().await.archive_session().await
    }

    pub async fn unarchive_session(&self) -> Result<(), SessionError> {
        self.inner.session.lock().await.unarchive_session().await
    }

    pub async fn delete_session(&self) -> Result<(), SessionError> {
        self.cancel();
        self.inner.session.lock().await.delete_session().await
    }
}

pub struct SessionSnapshot {
    setting: Setting,
    agents: Vec<AgentDefinition>,
    sandbox_mode: SandboxMode,
    mode: String,
    thought_level: Effort,
}

impl fmt::Debug for SessionSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionSnapshot")
            .field("mode", &self.mode)
            .field("sandbox_mode", &self.sandbox_mode)
            .field("thought_level", &self.thought_level)
            .finish_non_exhaustive()
    }
}

impl SessionSnapshot {
    fn new(session: &Session) -> Self {
        Self {
            setting: session.setting().clone(),
            agents: session.agents().to_vec(),
            sandbox_mode: session.sandbox_mode(),
            mode: session.mode(),
            thought_level: session.thought_level(),
        }
    }

    pub fn setting(&self) -> &Setting {
        &self.setting
    }

    pub fn agents(&self) -> &[AgentDefinition] {
        &self.agents
    }

    pub fn sandbox_mode(&self) -> SandboxMode {
        self.sandbox_mode
    }

    pub fn mode(&self) -> String {
        self.mode.clone()
    }

    pub fn thought_level(&self) -> Effort {
        self.thought_level
    }
}
