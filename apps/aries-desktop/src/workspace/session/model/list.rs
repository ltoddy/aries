use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use agent_client_protocol::Error;
use agent_client_protocol::v1::schema::{SessionId, SessionInfo, SessionInfoUpdate};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ListState {
    #[default]
    Disconnected,
    Connecting,
    Unsupported,
    Ready,
    Loading,
}

#[derive(Debug, Default)]
pub struct SessionList {
    entries: Vec<SessionInfo>,
    projects: BTreeMap<PathBuf, Vec<usize>>,
    indices: HashMap<SessionId, usize>,
    expanded_cwds: HashSet<PathBuf>,
    state: ListState,
    refresh_pending: bool,
    error: Option<String>,
}

impl SessionList {
    pub fn entries(&self) -> &[SessionInfo] {
        &self.entries
    }

    pub fn session(&self, id: &SessionId) -> Option<&SessionInfo> {
        self.indices.get(id).map(|&index| &self.entries[index])
    }

    pub fn projects(&self) -> impl Iterator<Item = &Path> {
        self.projects.keys().map(PathBuf::as_path)
    }

    pub fn project_sessions<P: AsRef<Path>>(
        &self,
        cwd: P,
    ) -> impl ExactSizeIterator<Item = &SessionInfo> + use<'_, P> {
        self.projects
            .get(cwd.as_ref())
            .map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .map(|&index| &self.entries[index])
    }

    fn rebuild_projects(&mut self) {
        self.projects.clear();
        for (index, session) in self.entries.iter().enumerate() {
            if let Some(indices) = self.projects.get_mut(session.cwd.as_path()) {
                indices.push(index);
            } else {
                self.projects.insert(session.cwd.clone(), vec![index]);
            }
        }
    }

    fn rebuild_indices(&mut self) {
        self.rebuild_projects();
        self.indices.clear();
        for (index, session) in self.entries.iter().enumerate() {
            self.indices.insert(session.session_id.clone(), index);
        }
    }

    pub fn is_supported(&self) -> bool {
        matches!(self.state, ListState::Ready | ListState::Loading)
    }

    pub fn is_unsupported(&self) -> bool {
        self.state == ListState::Unsupported
    }

    pub fn is_connecting(&self) -> bool {
        self.state == ListState::Connecting
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.state, ListState::Connecting | ListState::Loading)
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn connect(&mut self) {
        self.state = ListState::Connecting;
        self.refresh_pending = false;
        self.error = None;
    }

    pub fn connected(&mut self, supported: bool) {
        if self.state != ListState::Connecting {
            return;
        }
        self.state = if supported { ListState::Ready } else { ListState::Unsupported };
    }

    pub fn disconnect(&mut self, error: String) {
        self.state = ListState::Disconnected;
        self.refresh_pending = false;
        self.error = Some(error);
    }

    pub fn defer_refresh(&mut self) {
        self.refresh_pending = true;
    }

    pub fn take_refresh_request(&mut self) -> bool {
        std::mem::take(&mut self.refresh_pending)
    }

    pub fn begin_refresh(&mut self) -> bool {
        if self.state != ListState::Ready {
            return false;
        }
        self.state = ListState::Loading;
        self.error = None;
        true
    }

    pub fn fail_refresh(&mut self, error: String) {
        if self.state != ListState::Loading {
            return;
        }
        self.state = ListState::Ready;
        self.error = Some(error);
    }

    pub fn receive(&mut self, result: Result<Vec<SessionInfo>, Error>) {
        if self.state != ListState::Loading {
            return;
        }
        self.state = ListState::Ready;
        match result {
            Ok(mut sessions) => {
                self.indices.clear();
                sessions.retain(|session| {
                    if self.indices.contains_key(&session.session_id) {
                        return false;
                    }
                    self.indices.insert(session.session_id.clone(), self.indices.len());
                    true
                });
                self.entries = sessions;
                self.rebuild_projects();
                self.error = None;
            },
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn update(&mut self, id: &SessionId, update: &SessionInfoUpdate) {
        if let Some(&index) = self.indices.get(id) {
            let session = &mut self.entries[index];
            update.title.clone().update_to(&mut session.title);
            update.updated_at.clone().update_to(&mut session.updated_at);
        }
    }

    pub fn remove_sessions(&mut self, ids: &HashSet<SessionId>) {
        self.entries.retain(|session| !ids.contains(&session.session_id));
        self.rebuild_indices();
    }

    pub fn is_project_expanded(&self, cwd: impl AsRef<Path>) -> bool {
        self.expanded_cwds.contains(cwd.as_ref())
    }

    pub fn expand_project(&mut self, cwd: impl AsRef<Path>) {
        let cwd = cwd.as_ref();
        if !self.expanded_cwds.contains(cwd) {
            self.expanded_cwds.insert(cwd.to_owned());
        }
    }

    pub fn expand_all(&mut self) {
        for cwd in self.projects.keys() {
            if !self.expanded_cwds.contains(cwd.as_path()) {
                self.expanded_cwds.insert(cwd.clone());
            }
        }
    }

    pub fn collapse_all(&mut self) {
        self.expanded_cwds.clear();
    }

    pub fn toggle_project(&mut self, cwd: impl AsRef<Path>) {
        let cwd = cwd.as_ref();
        if !self.expanded_cwds.remove(cwd) {
            self.expanded_cwds.insert(cwd.to_owned());
        }
    }
}
