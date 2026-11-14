mod error;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aries_extension::mcp::McpDefinition;
use aries_init::{GlobalContext, Setting};
use aries_persistence::SessionRepository;
use toasty::Db;
use tokio::sync::Mutex;
use tracing::{Instrument, info_span};

pub use self::error::RegistryError;
use crate::session::Session;

pub type SharedRegistry = Arc<Mutex<SessionRegistry>>;

pub struct SessionRegistry {
    gctx: GlobalContext,
    setting: Setting,

    db: Db,
    sessions: HashMap<String, Session>,
    session_repo: SessionRepository,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, setting: Setting) -> Result<Self, RegistryError> {
        let root_dir = gctx.root_dir();
        let db = aries_persistence::connect(root_dir).await?;
        let session_repo = SessionRepository::new(db.clone());

        Ok(Self { gctx, setting, db, sessions: Default::default(), session_repo })
    }

    pub async fn list_sessions(
        &mut self,
        cwd: Option<PathBuf>,
    ) -> Result<Vec<aries_persistence::Session>, RegistryError> {
        let sessions = match &cwd {
            Some(cwd) => self.session_repo.find_by_cwd(cwd.display().to_string()).await,
            None => self.session_repo.find().await,
        }?;

        let sessions = sessions
            .into_iter()
            .filter(|s| PathBuf::from(&s.root_dir).exists())
            .collect::<Vec<_>>();

        Ok(sessions)
    }

    pub async fn delete_session(
        &mut self,
        session_id: impl AsRef<str>,
    ) -> Result<(), RegistryError> {
        let session_id = session_id.as_ref();

        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| RegistryError::session_not_found(session_id))?;
        session.delete_session().await.map_err(RegistryError::session)?;

        self.sessions.remove(session_id);

        Ok(())
    }

    pub async fn try_session(
        &mut self,
        cwd: impl Into<PathBuf>,
        session_id: impl AsRef<str>,
    ) -> Result<Session, RegistryError> {
        let cwd: PathBuf = cwd.into();
        let session_id = session_id.as_ref();
        if let Some(session) = self.sessions.get(session_id) {
            return Ok(session.to_owned());
        }

        let mcp_config = McpDefinition::empty();
        match self.session_repo.find_last_by_session_id(session_id).await? {
            Some(_) => self.load_session(session_id, mcp_config).await,
            None => self.new_session(cwd, mcp_config).await,
        }
    }

    pub fn session(&self, session_id: impl AsRef<str>) -> Option<Session> {
        let session = self.sessions.get(session_id.as_ref())?;

        Some(session.to_owned())
    }

    pub fn putback_session(&mut self, session: Session) {
        if let Some(active_session) = self.sessions.get_mut(&session.id()) {
            *active_session = session;
        }
    }

    pub async fn new_session(
        &mut self,
        cwd: impl Into<PathBuf>,
        external_mcp_config: McpDefinition,
    ) -> Result<Session, RegistryError> {
        let cwd: PathBuf = cwd.into();
        let session_id = nanoid::nanoid!();
        let model_config = self.setting.active_model()?;

        let session = Session::new(
            &session_id,
            self.gctx.clone(),
            cwd,
            model_config,
            self.setting.clone(),
            self.db.clone(),
            external_mcp_config,
        )
        .instrument(info_span!("session_init", session_id = %session_id))
        .await?;

        self.session_repo
            .create(
                session.id(),
                session.current_dir().display().to_string(),
                session.session_dir().display().to_string(),
                session.transcript_path().display().to_string(),
            )
            .await?;
        self.sessions.insert(session.id(), session.clone());

        Ok(session)
    }

    pub async fn load_session(
        &mut self,
        session_id: impl AsRef<str>,
        external_mcp_config: McpDefinition,
    ) -> Result<Session, RegistryError> {
        let session_id = session_id.as_ref();

        let session = self
            .session_repo
            .find_last_by_session_id(session_id)
            .await?
            .ok_or_else(|| toasty::Error::record_not_found(format!("session {session_id}")))?;

        let model_config = self.setting.active_model()?;

        let session = Session::load(
            &session.session_id,
            self.gctx.clone(),
            session.cwd,
            model_config,
            self.setting.clone(),
            self.db.clone(),
            external_mcp_config,
        )
        .instrument(info_span!("session_init", session_id = %session_id))
        .await?;

        self.sessions.insert(session.id(), session.clone());

        Ok(session)
    }

    pub async fn fork_session(
        &mut self,
        parent_session_id: impl AsRef<str>,
        cwd: impl Into<PathBuf>,
        external_mcp_config: McpDefinition,
    ) -> Result<Session, RegistryError> {
        let parent_session_id = parent_session_id.as_ref();
        let cwd: PathBuf = cwd.into();
        let session_id = nanoid::nanoid!();

        let parent_session = self
            .sessions
            .get(parent_session_id)
            .ok_or_else(|| RegistryError::session_not_active(parent_session_id))?;

        let model_config = self.setting.active_model()?;
        let session = Session::new(
            &session_id,
            self.gctx.clone(),
            cwd,
            model_config,
            self.setting.clone(),
            self.db.clone(),
            external_mcp_config,
        )
        .instrument(info_span!("session_init", session_id = %session_id))
        .await?;

        let parent_context = parent_session.context().await;
        session.overwrite_context(parent_context).await;

        self.session_repo
            .create(
                &session.id(),
                session.current_dir().display().to_string(),
                session.session_dir().display().to_string(),
                session.transcript_path().display().to_string(),
            )
            .await?;
        self.sessions.insert(session.id(), session.clone());

        Ok(session)
    }
}
