mod error;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use aries_extension::mcp::McpDefinition;
use aries_init::{GlobalContext, Setting};
use aries_persistence::SessionRepository;
use toasty::Db;
use tokio::sync::Mutex;

pub use self::error::RegistryError;
use crate::session::SessionHandle;

pub type SharedRegistry = Arc<Mutex<SessionRegistry>>;

pub struct SessionRegistry {
    gctx: GlobalContext,
    setting: Setting,

    db: Db,
    sessions: HashMap<String, SessionHandle>,
    session_repo: SessionRepository,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, setting: Setting) -> Result<Self, RegistryError> {
        let root_dir = gctx.root_dir();
        let db = aries_persistence::connect(root_dir).await.map_err(RegistryError::db)?;
        let session_repo = SessionRepository::new(db.clone());

        Ok(Self { gctx, setting, db, sessions: Default::default(), session_repo })
    }

    pub async fn new_session(
        &mut self,
        cwd: impl Into<PathBuf>,
        external_mcp_config: McpDefinition,
    ) -> Result<SessionHandle, RegistryError> {
        let cwd: PathBuf = cwd.into();
        let model_config = self.setting.active_model().map_err(RegistryError::setting)?;

        let session = SessionHandle::new_session(
            self.gctx.clone(),
            cwd,
            model_config,
            self.setting.clone(),
            self.db.clone(),
            external_mcp_config,
        )
        .await
        .map_err(RegistryError::session)?;
        self.sessions.insert(session.id(), session.clone());

        Ok(session)
    }

    pub async fn load_session(
        &mut self,
        session_id: impl AsRef<str>,
        external_mcp_config: McpDefinition,
    ) -> Result<SessionHandle, RegistryError> {
        let session_id = session_id.as_ref();
        if self.sessions.contains_key(session_id) {
            return Err(RegistryError::session_already_active(session_id));
        }
        let model_config = self.setting.active_model().map_err(RegistryError::setting)?;

        let session = SessionHandle::load_session(
            session_id,
            self.gctx.clone(),
            model_config,
            self.setting.clone(),
            self.db.clone(),
            external_mcp_config,
        )
        .await
        .map_err(RegistryError::session)?;

        self.sessions.insert(session.id(), session.clone());

        Ok(session)
    }

    pub async fn resume_session(
        &mut self,
        session_id: impl AsRef<str>,
        external_mcp_config: McpDefinition,
    ) -> Result<SessionHandle, RegistryError> {
        let session_id = session_id.as_ref();
        if self.sessions.contains_key(session_id) {
            return Err(RegistryError::session_already_active(session_id));
        }
        let model_config = self.setting.active_model().map_err(RegistryError::setting)?;

        let session = SessionHandle::resume_session(
            session_id,
            self.gctx.clone(),
            model_config,
            self.setting.clone(),
            self.db.clone(),
            external_mcp_config,
        )
        .await
        .map_err(RegistryError::session)?;

        self.sessions.insert(session.id(), session.clone());

        Ok(session)
    }

    pub async fn fork_session(
        &mut self,
        parent_session_id: impl AsRef<str>,
        cwd: impl Into<PathBuf>,
        external_mcp_config: McpDefinition,
    ) -> Result<SessionHandle, RegistryError> {
        let parent_session_id = parent_session_id.as_ref();
        let cwd: PathBuf = cwd.into();

        let parent_session = self
            .sessions
            .get(parent_session_id)
            .ok_or_else(|| RegistryError::session_not_active(parent_session_id))?;

        let parent_context = parent_session.context().await.map_err(RegistryError::session)?;
        let model_config = self.setting.active_model().map_err(RegistryError::setting)?;
        let session = SessionHandle::new_session(
            self.gctx.clone(),
            cwd,
            model_config,
            self.setting.clone(),
            self.db.clone(),
            external_mcp_config,
        )
        .await
        .map_err(RegistryError::session)?;

        session.overwrite_context(parent_context).await;
        self.sessions.insert(session.id(), session.clone());

        Ok(session)
    }

    pub async fn try_session(
        &mut self,
        cwd: impl Into<PathBuf>,
        session_id: impl AsRef<str>,
    ) -> Result<SessionHandle, RegistryError> {
        let cwd: PathBuf = cwd.into();
        let session_id = session_id.as_ref();
        if let Some(session) = self.sessions.get(session_id) {
            return Ok(session.to_owned());
        }

        let mcp_config = McpDefinition::empty();
        match self
            .session_repo
            .find_last_by_session_id(session_id)
            .await
            .map_err(RegistryError::db)?
        {
            Some(_) => self.load_session(session_id, mcp_config).await,
            None => self.new_session(cwd, mcp_config).await,
        }
    }

    pub fn session(&self, session_id: impl AsRef<str>) -> Option<SessionHandle> {
        let session = self.sessions.get(session_id.as_ref())?;

        Some(session.to_owned())
    }

    pub async fn list_sessions(
        &mut self,
        cwd: Option<PathBuf>,
    ) -> Result<Vec<aries_persistence::Session>, RegistryError> {
        let sessions = match &cwd {
            Some(cwd) => self.session_repo.find_by_cwd(cwd.display().to_string()).await,
            None => self.session_repo.find().await,
        }
        .map_err(RegistryError::db)?;

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
        if let Some(session) = self.session(session_id) {
            session.delete_session().await.map_err(RegistryError::session)?;
            self.sessions.remove(session_id);
            return Ok(());
        }
        let model_config = self.setting.active_model().map_err(RegistryError::setting)?;

        let session = SessionHandle::resume_session(
            session_id,
            self.gctx.clone(),
            model_config,
            self.setting.clone(),
            self.db.clone(),
            McpDefinition::empty(),
        )
        .await
        .map_err(RegistryError::session)?;
        session.delete_session().await.map_err(RegistryError::session)?;
        self.sessions.remove(session_id);

        Ok(())
    }
}
