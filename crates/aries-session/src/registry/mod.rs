use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use aries_config::AriesConfig;
use aries_context::GlobalContext;

use crate::Session;
use crate::persistence::{ProjectRepository, SessionRepository};

pub struct SessionRegistry {
    gctx: GlobalContext,
    config: AriesConfig,

    active_sessions: HashMap<String, Session>,

    project_repo: ProjectRepository,
    session_repo: SessionRepository,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, config: AriesConfig) -> anyhow::Result<Self> {
        let mut db = crate::persistence::connect(&gctx.config_dir).await?;
        let project_repo = ProjectRepository::new(db.clone());
        let session_repo = SessionRepository::new(db.clone());

        let active_sessions = HashMap::new();

        let _ = crate::initalize_tables(&mut db).await;

        let registry = Self { gctx, config, active_sessions, project_repo, session_repo };
        Ok(registry)
    }

    pub async fn active(
        &mut self,
        dir: impl AsRef<Path>,
    ) -> anyhow::Result<crate::persistence::Project> {
        let dir = dir.as_ref();

        let name =
            dir.file_name().ok_or_else(|| anyhow!("Unable to recognize the directory name"))?;
        let name = name.to_string_lossy().to_string();

        let project = self.project_repo.upsert_by_dir(dir.display().to_string(), name).await?;

        Ok(project)
    }

    pub async fn list_projects(&mut self) -> anyhow::Result<Vec<crate::persistence::Project>> {
        let projects = self.project_repo.all().await?;
        Ok(projects)
    }

    pub async fn list_sessions(
        &mut self,
        project_id: u64,
    ) -> anyhow::Result<Vec<crate::persistence::Session>> {
        let sessions = self.session_repo.find_by_project_id(project_id).await?;
        Ok(sessions)
    }

    pub async fn get_session(
        &mut self,
        project: crate::persistence::Project,
        session_id: String,
    ) -> anyhow::Result<Session> {
        if let Some(session) = self.active_sessions.get(&session_id) {
            return Ok(session.to_owned());
        }

        match self.session_repo.find_last_by_session_id(&session_id).await {
            Ok(s) => self.load_session(s).await,
            Err(_) => self.create_session(project, Some(session_id)).await,
        }
    }

    async fn load_session(&mut self, s: crate::persistence::Session) -> anyhow::Result<Session> {
        let root = PathBuf::from(s.root_dir);
        let session = Session::load(s.session_id, &root, self.config.clone()).await?;
        self.active_sessions.insert(session.id(), session.clone());

        Ok(session)
    }

    async fn create_session(
        &mut self,
        project: crate::persistence::Project,
        session_id: Option<String>,
    ) -> anyhow::Result<Session> {
        let session_id = session_id.unwrap_or(nanoid::nanoid!());

        let gctx = GlobalContext { current_dir: PathBuf::from(project.dir), ..self.gctx.clone() };

        let session = Session::new(session_id, gctx, self.config.clone()).await?;

        let root = session.dir().display().to_string();

        let _ = self.session_repo.create(&session.id(), "", &root, project.id).await;

        self.active_sessions.insert(session.id(), session.clone());

        Ok(session)
    }
}
