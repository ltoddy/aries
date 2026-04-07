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
    context: GlobalContext,
    config: AriesConfig,
    hook: H,
}

impl SessionManager<()> {
    pub fn new(context: GlobalContext, config: AriesConfig) -> Self {
        Self { sessions: HashMap::new(), active_session_id: None, context, config, hook: () }
    }
}

impl<H> SessionManager<H>
where
    H: PromptHook<openai::CompletionModel> + PromptHook<azure::CompletionModel> + Clone + 'static,
{
    pub fn new_with_task_hook(context: GlobalContext, config: AriesConfig, hook: H) -> Self {
        Self { sessions: HashMap::new(), active_session_id: None, context, config, hook }
    }

    pub fn create_session(&mut self) -> anyhow::Result<String> {
        let session_id = nanoid::nanoid!();
        let session =
            Session::new_with_task_hook(session_id.clone(), &self.context, self.config.clone(), self.hook.clone())?;
        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id.clone());
        Ok(session_id)
    }

    pub fn insert_session(&mut self, session_id: String) -> anyhow::Result<()> {
        let session =
            Session::new_with_task_hook(session_id.clone(), &self.context, self.config.clone(), self.hook.clone())?;
        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id);
        Ok(())
    }

    pub fn get_session(&self, session_id: &str) -> Option<&Session<H>> {
        self.sessions.get(session_id)
    }

    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut Session<H>> {
        self.sessions.get_mut(session_id)
    }

    pub fn get_active_session(&self) -> Option<&Session<H>> {
        self.active_session_id.as_ref().and_then(|id| self.sessions.get(id))
    }

    pub fn get_active_session_mut(&mut self) -> Option<&mut Session<H>> {
        self.active_session_id.as_ref().and_then(|id| self.sessions.get_mut(id))
    }

    pub fn set_active_session(&mut self, session_id: &str) -> bool {
        if self.sessions.contains_key(session_id) {
            self.active_session_id = Some(session_id.to_string());
            true
        } else {
            false
        }
    }

    pub fn delete_session(&mut self, session_id: &str) -> bool {
        if self.sessions.remove(session_id).is_some() {
            if self.active_session_id.as_deref() == Some(session_id) {
                self.active_session_id = self.sessions.keys().next().cloned();
            }
            true
        } else {
            false
        }
    }

    pub fn list_sessions(&self) -> Vec<&Session<H>> {
        self.sessions.values().collect()
    }

    pub fn get_active_session_id(&self) -> Option<&String> {
        self.active_session_id.as_ref()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}
