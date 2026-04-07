use std::collections::HashMap;

use aries_config::AriesConfig;
use aries_context::GlobalContext;

use crate::Session;

pub struct SessionManager {
    sessions: HashMap<String, Session>,
    active_session_id: Option<String>,
    context: GlobalContext,
    config: AriesConfig,
}

impl SessionManager {
    pub fn new(context: GlobalContext, config: AriesConfig) -> Self {
        Self { sessions: HashMap::new(), active_session_id: None, context, config }
    }

    pub fn create_session(&mut self) -> anyhow::Result<String> {
        let session_id = nanoid::nanoid!();
        let session = Session::new(session_id.clone(), &self.context, self.config.clone())?;
        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id.clone());
        Ok(session_id)
    }

    pub fn insert_session(&mut self, session_id: String) -> anyhow::Result<()> {
        let session = Session::new(session_id.clone(), &self.context, self.config.clone())?;
        self.sessions.insert(session_id.clone(), session);
        self.active_session_id = Some(session_id);
        Ok(())
    }

    pub fn get_session(&self, session_id: &str) -> Option<&Session> {
        self.sessions.get(session_id)
    }

    pub fn get_session_mut(&mut self, session_id: &str) -> Option<&mut Session> {
        self.sessions.get_mut(session_id)
    }

    pub fn get_active_session(&self) -> Option<&Session> {
        self.active_session_id.as_ref().and_then(|id| self.sessions.get(id))
    }

    pub fn get_active_session_mut(&mut self) -> Option<&mut Session> {
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

    pub fn list_sessions(&self) -> Vec<&Session> {
        self.sessions.values().collect()
    }

    pub fn get_active_session_id(&self) -> Option<&String> {
        self.active_session_id.as_ref()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }
}
