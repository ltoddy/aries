use std::collections::HashMap;

use aries_config::AriesConfig;
use aries_context::GlobalContext;
use rig::agent::PromptHook;
use rig::providers::{azure, openai};

use crate::Session;

pub struct SessionManager<H = ()>
where
    H: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel>,
{
    sessions: HashMap<String, Session<H>>,
    active_session_id: Option<String>,
    gctx: GlobalContext,
    config: AriesConfig,
    hook: H,
}

impl<H> SessionManager<H>
where
    H: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel> + Clone + 'static,
{
    pub fn new(gctx: GlobalContext, config: AriesConfig, hook: H) -> Self {
        Self { sessions: HashMap::new(), active_session_id: None, gctx, config, hook }
    }

    pub async fn create_session(&mut self) -> anyhow::Result<String> {
        let session_id = nanoid::nanoid!();
        let session =
            Session::new(session_id.clone(), &self.gctx, self.config.clone(), self.hook.clone())
                .await?;
        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id.clone());
        Ok(session_id)
    }

    pub fn get_session(&self, session_id: &str) -> Option<&Session<H>> {
        self.sessions.get(session_id)
    }

    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut Session<H>> {
        self.sessions.get_mut(session_id)
    }
}
