use std::collections::HashMap;
use std::path::PathBuf;

use aries_extension::mcp::McpDefinition;
use aries_init::{GlobalContext, Setting, SettingError};
use aries_persistence::SessionRepository;
use toasty::Db;
use tracing::{Instrument, info_span};

use crate::session::Session;
use crate::session::session::SessionError;

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("{0}")]
    Db(#[from] toasty::Error),

    #[error("{0}")]
    Setting(#[from] SettingError),

    #[error("{0}")]
    Session(#[from] SessionError),

    #[error("parent session {0} is not active")]
    SessionNotActive(String),
}

impl RegistryError {
    pub fn session_not_active(session_id: impl Into<String>) -> Self {
        Self::SessionNotActive(session_id.into())
    }
}

pub struct SessionRegistry {
    gctx: GlobalContext,
    setting: Setting,

    db: Db,
    active_sessions: HashMap<String, Session>,
    session_repo: SessionRepository,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, setting: Setting) -> Result<Self, RegistryError> {
        let root_dir = gctx.root_dir();
        let db = aries_persistence::connect(&root_dir).await?;
        let session_repo = SessionRepository::new(db.clone());

        Ok(Self { gctx, setting, db, active_sessions: Default::default(), session_repo })
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

    pub async fn close_session(&mut self, session_id: impl AsRef<str>) {
        let session_id = session_id.as_ref();

        if let Some(removed) = self.active_sessions.remove(session_id) {
            removed.close().await;
        };
    }

    pub async fn delete_session(
        &mut self,
        session_id: impl AsRef<str>,
    ) -> Result<(), RegistryError> {
        let session_id = session_id.as_ref();

        if let Some(removed) = self.active_sessions.remove(session_id) {
            removed.close().await;
        }

        if let Ok(session) = self.session_repo.find_last_by_session_id(session_id).await {
            self.session_repo.delete_by_session_id(session_id).await?;
            let _ = tokio::fs::remove_dir_all(&session.root_dir).await;
        }

        Ok(())
    }

    pub async fn try_session(
        &mut self,
        cwd: impl Into<PathBuf>,
        session_id: impl AsRef<str>,
    ) -> Result<Session, RegistryError> {
        let cwd: PathBuf = cwd.into();
        let session_id = session_id.as_ref();
        if let Some(session) = self.active_sessions.get(session_id) {
            return Ok(session.to_owned());
        }

        let mcp_config = McpDefinition::empty();
        match self.session_repo.find_last_by_session_id(session_id).await {
            Ok(_) => self.load_session(session_id, mcp_config).await,
            Err(_) => self.new_session(cwd, mcp_config).await,
        }
    }

    pub fn session(&self, session_id: impl AsRef<str>) -> Option<Session> {
        let session = self.active_sessions.get(session_id.as_ref())?;

        Some(session.to_owned())
    }

    pub fn putback_session(&mut self, session: Session) {
        self.active_sessions.insert(session.id(), session);
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

        self.active_sessions.insert(session.id(), session.clone());
        self.session_repo
            .create(
                session.id(),
                session.current_dir().display().to_string(),
                session.session_dir().display().to_string(),
                session.transcript_path().display().to_string(),
            )
            .await?;

        Ok(session)
    }

    pub async fn load_session(
        &mut self,
        session_id: impl AsRef<str>,
        external_mcp_config: McpDefinition,
    ) -> Result<Session, RegistryError> {
        let session_id = session_id.as_ref();

        let session = self.session_repo.find_last_by_session_id(session_id).await?;

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

        self.active_sessions.insert(session.id(), session.clone());

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
            .active_sessions
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

        self.active_sessions.insert(session.id(), session.clone());
        self.session_repo
            .create(
                &session.id(),
                session.current_dir().display().to_string(),
                session.session_dir().display().to_string(),
                session.transcript_path().display().to_string(),
            )
            .await?;

        Ok(session)
    }
}
