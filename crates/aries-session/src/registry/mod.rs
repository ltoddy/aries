use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use aries_context::GlobalContext;
use aries_extension::hook::{HooksExecutor, HooksLoader};
use aries_init::Setting;

use crate::Session;
use crate::persistence::SessionRepository;

pub struct SessionRegistry {
    gctx: GlobalContext,
    setting: Setting,

    active_sessions: HashMap<String, Session>,
    session_repo: SessionRepository,

    hooks_executor: Arc<HooksExecutor>,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, setting: Setting) -> anyhow::Result<Self> {
        #[rustfmt::skip]
        let mut db = crate::persistence::connect(&gctx.root_dir)
            .await
            .with_context(|| format!("connecting to session database at {}", gctx.root_dir.display()))?;
        let _ = crate::migrate(&mut db).await;

        let session_repo = SessionRepository::new(db.clone());

        let mut hooks_loader = HooksLoader::new(&gctx.current_dir);
        let hooks = hooks_loader.load().await.unwrap_or_default();
        let hooks_executor = Arc::new(HooksExecutor::new(hooks));

        Ok(Self {
            gctx,
            setting,
            active_sessions: Default::default(),
            session_repo,
            hooks_executor,
        })
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

        let sessions = sessions
            .into_iter()
            .filter(|s| PathBuf::from(&s.root_dir).exists())
            .collect::<Vec<_>>();

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
        let root_dir = self.gctx.root_dir.join(format!("{}{session_id}", Session::PREFIX));

        let model_config = self.setting.active_model()?;

        let session = Session::new(
            session_id,
            &root_dir,
            &cwd,
            model_config,
            self.setting.clone(),
            self.hooks_executor.clone(),
        )
        .await
        .with_context(|| format!("Failed to create session at {}", root_dir.display()))?;

        self.active_sessions.insert(session.id(), session.clone());

        self.session_repo
            .create(&session.id(), &cwd, root_dir.display().to_string())
            .await
            .with_context(|| "Failed to create session info in local storage")?;

        Ok(session)
    }

    pub async fn load_session(&mut self, session_id: impl Into<String>) -> anyhow::Result<Session> {
        let session_id = session_id.into();

        let session = self
            .session_repo
            .find_last_by_session_id(&session_id)
            .await
            .with_context(|| format!("Failed to load session {session_id} from database"))?;

        let root_dir = self.gctx.root_dir.join(format!("{}{session_id}", Session::PREFIX));

        let model_config = self.setting.active_model()?;

        let session = Session::load(
            session.session_id,
            &root_dir,
            session.cwd,
            model_config,
            self.setting.clone(),
            self.hooks_executor.clone(),
        )
        .await
        .with_context(|| format!("Failed to load session from: {}", root_dir.display()))?;

        self.active_sessions.insert(session.id(), session.clone());

        Ok(session)
    }
}
