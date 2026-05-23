use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use aries_config::AriesConfig;
use aries_context::GlobalContext;
use aries_core::ext::hook::{HooksLoader, HooksPreset};

use crate::Session;
use crate::persistence::SessionRepository;

pub struct SessionRegistry {
    gctx: GlobalContext,
    config: AriesConfig,

    active_sessions: HashMap<String, Session>,
    session_repo: SessionRepository,

    #[allow(unused)]
    hooks: Vec<HooksPreset>,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, config: AriesConfig) -> anyhow::Result<Self> {
        let mut db = crate::persistence::connect(&gctx.config_dir).await.with_context(|| {
            format!("Failed to connect local storage: {}", gctx.config_dir.display())
        })?;
        let _ = crate::migrate(&mut db).await;

        let session_repo = SessionRepository::new(db.clone());

        let mut hooks_loader = HooksLoader::new(&gctx.current_dir);
        let hooks = hooks_loader.load().await.unwrap_or_default();

        Ok(Self { gctx, config, active_sessions: Default::default(), session_repo, hooks })
    }

    pub async fn list_projects(&mut self) -> anyhow::Result<Vec<String>> {
        let projects = self
            .session_repo
            .find_projects()
            .await
            .with_context(|| "Failed to list all project directories")?;
        Ok(projects)
    }

    pub async fn list_sessions(
        &mut self,
        cwd: Option<PathBuf>,
    ) -> anyhow::Result<Vec<crate::persistence::Session>> {
        let sessions = match cwd {
            Some(cwd) => self.session_repo.find_by_cwd(cwd.display().to_string()).await,
            None => self.session_repo.find().await,
        }
        .with_context(|| "Failed to find session info")?;

        Ok(sessions)
    }

    pub async fn try_session(
        &mut self,
        project_dir: &str,
        session_id: &str,
    ) -> anyhow::Result<Session> {
        if let Some(session) = self.active_sessions.get(session_id) {
            return Ok(session.to_owned());
        }

        match self.session_repo.find_last_by_session_id(session_id).await {
            Ok(_) => self.load_session(session_id).await,
            Err(_) => self.new_session(project_dir).await,
        }
    }

    pub fn get_session(&self, session_id: impl Into<String>) -> Option<Session> {
        let session = self.active_sessions.get(&session_id.into())?;

        Some(session.to_owned())
    }

    pub fn putback_session(&mut self, session: Session) {
        self.active_sessions.insert(session.id(), session);
    }

    pub async fn new_session(&mut self, cwd: impl Into<String>) -> anyhow::Result<Session> {
        let cwd = cwd.into();

        let session_id = nanoid::nanoid!();
        let root_dir = self.gctx.config_dir.join(format!("{}{session_id}", Session::PREFIX));

        let session = Session::new(session_id, self.config.clone(), &root_dir, &cwd)
            .await
            .with_context(|| format!("Failed to new session at {}", root_dir.display()))?;

        self.active_sessions.insert(session.id(), session.clone());

        self.session_repo
            .create(&session.id(), &cwd, root_dir.display().to_string())
            .await
            .with_context(|| "Failed to create session info to local storage")?;

        Ok(session)
    }

    pub async fn load_session(&mut self, session_id: &str) -> anyhow::Result<Session> {
        let session = self
            .session_repo
            .find_last_by_session_id(session_id)
            .await
            .with_context(|| format!("Failed to find session info: {session_id}"))?;

        let root_dir = self.gctx.config_dir.join(format!("{}{session_id}", Session::PREFIX));

        let session = Session::load(
            session.session_id,
            self.config.clone(),
            root_dir.clone(),
            PathBuf::from(session.cwd),
        )
        .await
        .with_context(|| format!("Failed to load session from: {}", root_dir.display()))?;

        self.active_sessions.insert(session.id(), session.clone());

        Ok(session)
    }
}
