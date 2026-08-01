use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use aries_extension::mcp::McpDefinition;
use aries_init::{GlobalContext, Setting};
use aries_persistence::SessionRepository;
use toasty::Db;
use tracing::{Instrument, info_span};

use crate::Session;
use crate::session::SessionArgs;

pub struct SessionRegistry {
    gctx: GlobalContext,
    setting: Setting,

    db: Db,
    active_sessions: HashMap<String, Session>,
    session_repo: SessionRepository,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, setting: Setting) -> anyhow::Result<Self> {
        let db = aries_persistence::connect(&gctx.root_dir)
            .await
            .with_context(|| format!("connecting to database at {}", gctx.root_dir.display()))?;
        let session_repo = SessionRepository::new(db.clone());

        Ok(Self { gctx, setting, db, active_sessions: Default::default(), session_repo })
    }

    pub async fn list_sessions(
        &mut self,
        cwd: Option<PathBuf>,
    ) -> anyhow::Result<Vec<aries_persistence::Session>> {
        let sessions = match &cwd {
            Some(cwd) => self.session_repo.find_by_cwd(cwd.display().to_string()).await,
            None => self.session_repo.find().await,
        }
        .with_context(|| match &cwd {
            Some(cwd) => format!("failed to find session info for cwd {}", cwd.display()),
            None => "failed to find session info".to_string(),
        })?;

        let sessions = sessions
            .into_iter()
            .filter(|s| PathBuf::from(&s.root_dir).exists())
            .collect::<Vec<_>>();

        Ok(sessions)
    }

    pub async fn close_session(&mut self, session_id: impl Into<String>) {
        let session_id = session_id.into();

        if let Some(removed) = self.active_sessions.remove(&session_id) {
            removed.close().await;
        };

        if let Ok(session) = self.session_repo.find_last_by_session_id(&session_id).await {
            let _ = self.session_repo.delete_by_session_id(&session_id).await;
            let _ = tokio::fs::remove_dir_all(&session.root_dir).await;
        }
    }

    pub async fn delete_session(&mut self, session_id: impl Into<String>) -> anyhow::Result<()> {
        let session_id = session_id.into();

        if let Some(removed) = self.active_sessions.remove(&session_id) {
            removed.close().await;
        }

        if let Ok(session) = self.session_repo.find_last_by_session_id(&session_id).await {
            self.session_repo
                .delete_by_session_id(&session_id)
                .await
                .with_context(|| format!("failed to delete session {session_id} from database"))?;
            let _ = tokio::fs::remove_dir_all(&session.root_dir).await;
        }

        Ok(())
    }

    pub async fn try_session(
        &mut self,
        cwd: impl AsRef<Path>,
        session_id: impl Into<String>,
        args: SessionArgs,
    ) -> anyhow::Result<Session> {
        let cwd = cwd.as_ref();
        let session_id = session_id.into();
        if let Some(session) = self.active_sessions.get(&session_id) {
            return Ok(session.to_owned());
        }

        let mcp_config = McpDefinition::empty();
        match self.session_repo.find_last_by_session_id(&session_id).await {
            Ok(_) => self.load_session(session_id, mcp_config).await,
            Err(_) => self.new_session(cwd, mcp_config, args).await,
        }
    }

    pub fn get_session(&self, session_id: impl Into<String>) -> Option<Session> {
        let session = self.active_sessions.get(&session_id.into())?;

        Some(session.to_owned())
    }

    pub fn putback_session(&mut self, session: Session) {
        self.active_sessions.insert(session.id(), session);
    }

    pub async fn new_session(
        &mut self,
        cwd: impl AsRef<Path>,
        external_mcp_config: McpDefinition,
        args: SessionArgs,
    ) -> anyhow::Result<Session> {
        let cwd = cwd.as_ref();
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
            args,
        )
        .instrument(info_span!("session_init", session_id = %session_id))
        .await
        .with_context(|| format!("failed to create session {}", session_id))?;

        self.active_sessions.insert(session.id(), session.clone());
        self.session_repo
            .create(
                &session.id(),
                cwd.display().to_string(),
                session.session_dir().display().to_string(),
                session.transcript_path().display().to_string(),
            )
            .await
            .with_context(|| {
                format!("failed to create session info in local storage for session {session_id}")
            })?;

        Ok(session)
    }

    pub async fn load_session(
        &mut self,
        session_id: impl Into<String>,
        external_mcp_config: McpDefinition,
    ) -> anyhow::Result<Session> {
        let session_id = session_id.into();

        let session = self
            .session_repo
            .find_last_by_session_id(&session_id)
            .await
            .with_context(|| format!("failed to load session {session_id} from database"))?;

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
        .await
        .with_context(|| format!("failed to load session {session_id}"))?;

        self.active_sessions.insert(session.id(), session.clone());

        Ok(session)
    }
}
