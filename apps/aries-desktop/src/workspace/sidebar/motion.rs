use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::base::motion::{self, Transition};
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::*;
use gpui_kit::*;

#[derive(Clone)]
pub struct SidebarRow {
    session: SessionInfo,
    pending_created: bool,
}

impl SidebarRow {
    pub fn new(session: SessionInfo) -> Self {
        Self { session, pending_created: false }
    }

    pub fn session(&self) -> &SessionInfo {
        &self.session
    }
}

#[derive(Default)]
pub struct SidebarMotion {
    seen: HashSet<SessionId>,
    deleted: HashSet<SessionId>,
    rows: Vec<SidebarRow>,
}

impl SidebarMotion {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_title(&mut self, id: &SessionId, title: &str) {
        for row in &mut self.rows {
            if &row.session.session_id == id {
                row.session.title = Some(title.to_owned());
            }
        }
    }
    pub fn remove_session(&mut self, id: &SessionId) {
        self.rows.retain(|row| &row.session.session_id != id);
    }
    pub fn clear_rows(&mut self) {
        self.rows.clear();
    }
    pub fn receive(&mut self, sessions: Vec<SessionInfo>, confirmed: &HashSet<SessionId>) {
        self.reconcile(sessions);
        for row in &mut self.rows {
            if confirmed.contains(&row.session.session_id) {
                row.pending_created = false;
            }
        }
    }

    pub fn reconcile(&mut self, sessions: Vec<SessionInfo>) {
        let mut previous_rows = HashMap::with_capacity(self.rows.len());
        for row in &self.rows {
            previous_rows.entry(&row.session.session_id).or_insert(row);
        }
        let mut ids = HashSet::with_capacity(sessions.len());
        let mut rows = Vec::with_capacity(sessions.len());
        for session in sessions {
            let id = &session.session_id;
            if self.deleted.contains(id) || !ids.insert(id.clone()) {
                continue;
            }
            let previous = previous_rows.get(id).copied();
            self.seen.insert(id.clone());
            let pending_created = previous.is_some_and(|row| row.pending_created);
            let mut row = SidebarRow::new(session);
            row.pending_created = pending_created;
            rows.push(row);
        }
        for (position, row) in self.rows.iter().enumerate() {
            if row.pending_created && !ids.contains(&row.session.session_id) {
                rows.insert(position.min(rows.len()), row.clone());
            }
        }
        self.rows = rows;
    }

    pub fn add_created(&mut self, session: SessionInfo) {
        if self.deleted.contains(&session.session_id)
            || !self.seen.insert(session.session_id.clone())
        {
            return;
        }
        let mut row = SidebarRow::new(session);
        row.pending_created = true;
        self.rows.insert(0, row);
    }

    pub fn delete(&mut self, ids: &HashSet<SessionId>) {
        self.deleted.extend(ids.iter().cloned());
        self.rows.retain(|row| !ids.contains(&row.session.session_id));
    }

    pub fn projects(&self) -> BTreeMap<&Path, Vec<&SidebarRow>> {
        let mut projects = BTreeMap::new();
        for row in &self.rows {
            projects.entry(row.session.cwd.as_path()).or_insert_with(Vec::new).push(row);
        }
        projects
    }

    pub fn session_ids(&self, cwd: Option<&Path>) -> Vec<SessionId> {
        self.rows
            .iter()
            .filter(|row| cwd.is_none_or(|cwd| row.session.cwd == cwd))
            .map(|row| row.session.session_id.clone())
            .collect()
    }

    pub fn is_deleted(&self, id: &SessionId) -> bool {
        self.deleted.contains(id)
    }
}

#[derive(IntoElement)]
pub struct SessionSurface {
    row: SidebarRow,
    selected: bool,
    background: Hsla,
    child: AnyElement,
}

impl SessionSurface {
    pub fn new(row: SidebarRow, selected: bool, background: Hsla, child: AnyElement) -> Self {
        Self { row, selected, background, child }
    }
}

impl RenderOnce for SessionSurface {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let tokens = cx.theme().motion_tokens();
        let selection = motion::transition(
            (SharedString::from(format!("session:{}", self.row.session.session_id)), "selection"),
            if self.selected { 1_f32 } else { 0_f32 },
            Transition::new(tokens.duration_fast).easing(tokens.easing_move.clone()),
            window,
            cx,
        );
        let mut background = self.background;
        background.a *= selection;
        div()
            .id(SharedString::from(format!("session-surface:{}", self.row.session.session_id)))
            .bg(background)
            .rounded(cx.theme().radius)
            .hover(|row| row.bg(if self.selected { background } else { cx.theme().accent }))
            .child(self.child)
    }
}
