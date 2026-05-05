use std::collections::HashMap;

use aries_config::AriesConfig;
use aries_context::GlobalContext;

use crate::registry::manifest::ManifestFile;

mod manifest;

use crate::Session;

pub struct SessionRegistry {
    gctx: GlobalContext,
    config: AriesConfig,
    manifest: ManifestFile,
    sessions: HashMap<String, Session<()>>,
    active_session_id: Option<String>,
}

impl SessionRegistry {
    pub async fn new(gctx: GlobalContext, config: AriesConfig) -> Self {
        let manifest = ManifestFile::new(&gctx.config_dir).await;

        Self { gctx, config, manifest, sessions: Default::default(), active_session_id: None }
    }

    pub async fn create_session(&mut self) -> anyhow::Result<String> {
        let session_id = nanoid::nanoid!();
        let session =
            Session::new(session_id.clone(), self.gctx.clone(), self.config.clone(), ()).await?;
        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id.clone());
        Ok(session_id)
    }

    pub async fn list_sessions(&self) -> anyhow::Result<Vec<Session<()>>> {
        let mut sessions = Vec::<Session<()>>::new();

        let mut entries = tokio::fs::read_dir(&self.gctx.config_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if let Some(id) = name.strip_prefix(Session::<()>::PREFIX) {
                let id = id.to_owned();
                println!("id is: {}", id);
                if let Ok(session) = Session::load(id, entry.path(), self.config.clone(), ()).await
                {
                    sessions.push(session);
                }
            }
        }

        Ok(sessions)
    }

    pub fn get_session(&self, session_id: &str) -> Option<&Session<()>> {
        self.sessions.get(session_id)
    }

    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut Session<()>> {
        self.sessions.get_mut(session_id)
    }

    fn load_sessions() -> HashMap<String, Session<()>> {
        let mut sessions = HashMap::new();

        sessions
    }
}
