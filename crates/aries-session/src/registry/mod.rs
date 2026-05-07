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
        project_dir: &str,
    ) -> anyhow::Result<Vec<crate::persistence::Session>> {
        let sessions = self.session_repo.find_by_project_dir(project_dir).await?;
        Ok(sessions)
    }

    pub async fn get_session(
        &mut self,
        project_dir: &str,
        session_id: &str,
    ) -> anyhow::Result<Session> {
        if let Some(session) = self.active_sessions.get(session_id) {
            return Ok(session.to_owned());
        }

        match self.session_repo.find_last_by_session_id(&session_id).await {
            Ok(s) => self.load_session(session_id).await,
            Err(_) => self.create_session(project_dir).await,
        }
    }

    pub async fn create_session(&mut self, project_dir: &str) -> anyhow::Result<Session> {
        let gctx = GlobalContext { current_dir: PathBuf::from(project_dir), ..self.gctx.clone() };

        let session_id = nanoid::nanoid!();
        let session = Session::new(session_id, gctx, self.config.clone()).await?;
        self.active_sessions.insert(session.id(), session.clone());

        let _ = self.session_repo.create(&session.id(), project_dir).await;

        Ok(session)
    }

    async fn load_session(&mut self, session_id: &str) -> anyhow::Result<Session> {
        let root = self.gctx.config_dir.join(format!("{}-{}", Session::PREFIX, session_id));
        let session = Session::load(session_id.to_owned(), root, self.config.clone()).await?;
        self.active_sessions.insert(session.id(), session.clone());

        Ok(session)
    }
}
