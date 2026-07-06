use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use aries_context::GlobalContext;
use aries_extension::hook::input::{SessionEndHookInput, SessionEndReason};
use aries_extension::hook::{HooksExecutor, HooksLoader};
use aries_extension::mcp::{McpConfig, McpConfigLoader};
use aries_init::Setting;
use aries_persistence::SessionRepository;
use toasty::Db;
use tracing::{Instrument, info_span};

use crate::Session;

pub struct SessionRegistry {
    gctx: GlobalContext,
    setting: Setting,

    db: Db,
    active_sessions: HashMap<String, Session>,
    session_repo: SessionRepository,

    hooks_executor: Arc<HooksExecutor>,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, setting: Setting) -> anyhow::Result<Self> {
        let mut db = aries_persistence::connect(&gctx.root_dir)
            .await
            .with_context(|| format!("connecting to database at {}", gctx.root_dir.display()))?;
        let _ = aries_persistence::migrate(&mut db).await;

        let session_repo = SessionRepository::new(db.clone());

        let mut hooks_loader = HooksLoader::new(&gctx.current_dir);
        let hooks = hooks_loader.load().await.unwrap_or_default();
        let hooks_executor = Arc::new(HooksExecutor::new(hooks));

        Ok(Self {
            gctx,
            setting,
            db,
            active_sessions: Default::default(),
            session_repo,
            hooks_executor,
        })
    }

    pub async fn list_sessions(
        &mut self,
        cwd: Option<PathBuf>,
    ) -> anyhow::Result<Vec<aries_persistence::Session>> {
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

    pub async fn close_session(&mut self, session_id: impl Into<String>) {
        let session_id = session_id.into();

        self.active_sessions.remove(&session_id);

        if let Ok(session) = self.session_repo.find_last_by_session_id(&session_id).await {
            let _ = self.session_repo.delete_by_session_id(&session_id).await;
            let _ = tokio::fs::remove_dir_all(&session.root_dir).await;

            let input =
                SessionEndHookInput::new(&session_id, session.cwd, SessionEndReason::Logout)
                    .transcript_path(session.transcript_path);
            self.hooks_executor.fire_session_end(input).await;
        }
    }

    pub async fn try_session(
        &mut self,
        project_dir: impl Into<String>,
        session_id: impl Into<String>,
    ) -> anyhow::Result<Session> {
        let project_dir = project_dir.into();
        let session_id = session_id.into();
        if let Some(session) = self.active_sessions.get(&session_id) {
            return Ok(session.to_owned());
        }

        let mcp_loader = McpConfigLoader::new(&project_dir);
        let mcp_config = mcp_loader.load().await.unwrap_or_default();

        match self.session_repo.find_last_by_session_id(&session_id).await {
            Ok(_) => self.load_session(session_id, mcp_config).await,
            Err(_) => self.new_session(project_dir, mcp_config).await,
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
        cwd: impl Into<String>,
        mcp_config: McpConfig,
    ) -> anyhow::Result<Session> {
        let cwd = cwd.into();
        let session_id = nanoid::nanoid!();
        let model_config = self.setting.active_model()?;

        let session = Session::new(
            &session_id,
            &self.gctx.root_dir,
            &cwd,
            model_config,
            self.setting.clone(),
            self.db.clone(),
            self.hooks_executor.clone(),
            mcp_config,
        )
        .instrument(info_span!("session_init", session_id = %session_id))
        .await
        .with_context(|| format!("Failed to create session {}", session_id))?;

        self.active_sessions.insert(session.id(), session.clone());
        self.session_repo
            .create(
                &session.id(),
                &cwd,
                session.session_dir().display().to_string(),
                session.transcript_path().display().to_string(),
            )
            .await
            .with_context(|| "Failed to create session info in local storage")?;

        Ok(session)
    }

    pub async fn load_session(
        &mut self,
        session_id: impl Into<String>,
        mcp_config: McpConfig,
    ) -> anyhow::Result<Session> {
        let session_id = session_id.into();

        let session = self
            .session_repo
            .find_last_by_session_id(&session_id)
            .await
            .with_context(|| format!("Failed to load session {session_id} from database"))?;

        let model_config = self.setting.active_model()?;

        let session = Session::load(
            &session.session_id,
            &self.gctx.root_dir,
            session.cwd,
            model_config,
            self.setting.clone(),
            self.db.clone(),
            self.hooks_executor.clone(),
            mcp_config,
        )
        .instrument(info_span!("session_init", session_id = %session_id))
        .await
        .with_context(|| format!("Failed to load session: {}", session_id))?;

        self.active_sessions.insert(session.id(), session.clone());

        Ok(session)
    }
}
