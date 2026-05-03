use std::collections::HashMap;

use aries_config::AriesConfig;
use aries_context::GlobalContext;
use rig::agent::PromptHook;
use rig::providers::{azure, openai};

use crate::Session;

pub struct SessionManager<P = ()>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel>,
{
    sessions: HashMap<String, Session<P>>,
    active_session_id: Option<String>,
    gctx: GlobalContext,
    config: AriesConfig,
    hook: P,
}

impl<P> SessionManager<P>
where
    P: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel> + Clone + 'static,
{
    pub fn new(gctx: GlobalContext, config: AriesConfig, hook: P) -> Self {
        Self { sessions: HashMap::new(), active_session_id: None, gctx, config, hook }
    }

    pub async fn create_session(&mut self) -> anyhow::Result<String> {
        let session_id = nanoid::nanoid!();
        let session = Session::new(
            session_id.clone(),
            self.gctx.clone(),
            self.config.clone(),
            self.hook.clone(),
        )
        .await?;
        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id.clone());
        Ok(session_id)
    }

    pub async fn list_sessions(&self) -> anyhow::Result<Vec<Session<P>>> {
        let mut sessions = Vec::<Session<P>>::new();

        let mut entries = tokio::fs::read_dir(&self.gctx.config_dir).await?;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();
            if let Some(id) = name.strip_prefix(Session::<P>::PREFIX) {
                let id = id.to_owned();
                println!("id is: {}", id);
                if let Ok(session) =
                    Session::load(id, entry.path(), self.config.clone(), self.hook.clone()).await
                {
                    sessions.push(session);
                }
            }
        }

        Ok(sessions)
    }

    pub fn get_session(&self, session_id: &str) -> Option<&Session<P>> {
        self.sessions.get(session_id)
    }

    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut Session<P>> {
        self.sessions.get_mut(session_id)
    }
}
