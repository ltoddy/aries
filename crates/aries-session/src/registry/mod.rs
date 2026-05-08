use std::collections::HashMap;
use std::path::PathBuf;

use aries_config::AriesConfig;
use aries_context::GlobalContext;

use crate::Session;
use crate::persistence::SessionRepository;

pub struct SessionRegistry {
    gctx: GlobalContext,
    config: AriesConfig,

    active_sessions: HashMap<String, Session>,
    session_repo: SessionRepository,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, config: AriesConfig) -> anyhow::Result<Self> {
        let mut db = crate::persistence::connect(&gctx.config_dir).await?;
        let session_repo = SessionRepository::new(db.clone());

        let active_sessions = HashMap::new();

        let _ = crate::migrate(&mut db).await;

        let registry = Self { gctx, config, active_sessions, session_repo };
        Ok(registry)
    }

    pub async fn list_projects(&mut self) -> anyhow::Result<Vec<String>> {
        let projects = self.session_repo.find_projects().await?;
        Ok(projects)
    }

    pub async fn list_sessions(
        &mut self,
        cwd: Option<PathBuf>,
    ) -> anyhow::Result<Vec<crate::persistence::Session>> {
        let sessions = match cwd {
            Some(cwd) => self.session_repo.find_by_cwd(cwd.display().to_string()).await?,
            None => self.session_repo.find().await?,
        };

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
        let session_id = session_id.into();
        let session = self.active_sessions.get(&session_id)?;
        Some(session.to_owned())
    }

    pub async fn new_session(&mut self, cwd: impl Into<String>) -> anyhow::Result<Session> {
        let cwd = cwd.into();

        let session_id = nanoid::nanoid!();
        let root_dir = self.gctx.config_dir.join(format!("session-{session_id}"));

        let session = Session::new(session_id, self.config.clone(), &root_dir, &cwd).await?;
        self.active_sessions.insert(session.id(), session.clone());

        let _ = self.session_repo.create(&session.id(), &cwd, root_dir.display().to_string()).await;

        Ok(session)
    }

    pub async fn load_session(&mut self, session_id: &str) -> anyhow::Result<Session> {
        let session = self.session_repo.find_last_by_session_id(session_id).await?;

        let root_dir = self.gctx.config_dir.join(format!("session-{session_id}"));

        let session = Session::load(
            session.session_id,
            self.config.clone(),
            root_dir,
            PathBuf::from(session.cwd),
        )
        .await?;

        self.active_sessions.insert(session.id(), session.clone());

        Ok(session)
    }
}
