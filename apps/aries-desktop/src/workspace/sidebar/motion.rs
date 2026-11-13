use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::{Duration, Instant};

use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::base::motion::{self, Presence, Transition};
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;

#[derive(Clone)]
pub struct SidebarRow {
    session: SessionInfo,
    entered_at: Option<Instant>,
    exit: Option<(Instant, Duration, f32)>,
    pending_created: bool,
}

impl SidebarRow {
    pub fn session(&self) -> &SessionInfo {
        &self.session
    }
    pub fn is_exiting(&self) -> bool {
        self.exit.is_some()
    }

    fn opacity(&self, now: Instant, duration: Duration) -> f32 {
        if let Some((start, duration, from)) = self.exit {
            return from * (1. - progress(now, start, duration));
        }
        self.entered_at.map_or(1., |start| progress(now, start, duration))
    }
}

fn progress(now: Instant, start: Instant, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 1.;
    }
    let t =
        (now.saturating_duration_since(start).as_secs_f32() / duration.as_secs_f32()).clamp(0., 1.);
    1. - (1. - t).powi(3)
}

#[derive(Default)]
pub struct SidebarMotion {
    initialized: bool,
    seen: HashSet<SessionId>,
    deleted: HashSet<SessionId>,
    rows: Vec<SidebarRow>,
}

impl SidebarMotion {
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
    pub fn receive(
        &mut self,
        sessions: Vec<SessionInfo>,
        confirmed: &HashSet<SessionId>,
        now: Instant,
    ) {
        self.reconcile(sessions, now);
        for row in &mut self.rows {
            if confirmed.contains(&row.session.session_id) {
                row.pending_created = false;
            }
        }
        self.initialized = true;
    }

    pub fn reconcile(&mut self, sessions: Vec<SessionInfo>, now: Instant) {
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
            let entered_at = previous.map_or_else(
                || (self.seen.insert(id.clone()) && self.initialized).then_some(now),
                |row| row.entered_at,
            );
            self.seen.insert(id.clone());
            let pending_created = previous.is_some_and(|row| row.pending_created);
            rows.push(SidebarRow { session, entered_at, exit: None, pending_created });
        }
        for (position, row) in self.rows.iter().enumerate() {
            if row.pending_created && !row.is_exiting() && !ids.contains(&row.session.session_id) {
                rows.insert(position.min(rows.len()), row.clone());
            } else if row.is_exiting() {
                let group_position = self.rows[..position]
                    .iter()
                    .filter(|previous| previous.session.cwd == row.session.cwd)
                    .count();
                let position = rows
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| candidate.session.cwd == row.session.cwd)
                    .nth(group_position)
                    .map(|(position, _)| position)
                    .or_else(|| {
                        rows.iter()
                            .rposition(|candidate| candidate.session.cwd == row.session.cwd)
                            .map(|position| position + 1)
                    })
                    .unwrap_or(rows.len());
                rows.insert(position, row.clone());
            }
        }
        self.rows = rows;
    }

    pub fn add_created(&mut self, session: SessionInfo, now: Instant) {
        if self.deleted.contains(&session.session_id)
            || !self.seen.insert(session.session_id.clone())
        {
            return;
        }
        self.rows.insert(
            0,
            SidebarRow { session, entered_at: Some(now), exit: None, pending_created: true },
        );
    }

    pub fn delete(&mut self, ids: &HashSet<SessionId>, now: Instant, duration: Duration) {
        self.deleted.extend(ids.iter().cloned());
        for row in &mut self.rows {
            if ids.contains(&row.session.session_id) && !row.is_exiting() {
                let opacity = row.opacity(now, duration);
                row.exit = Some((now, duration, opacity));
            }
        }
        self.cleanup(now, duration.is_zero());
    }

    pub fn cleanup(&mut self, now: Instant, reduced: bool) {
        self.rows.retain(|row| {
            row.exit.is_none_or(|(start, duration, _)| {
                !reduced && now.saturating_duration_since(start) < duration
            })
        });
        if reduced {
            for row in &mut self.rows {
                row.entered_at = None;
            }
        }
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
            .filter(|row| !row.is_exiting() && cwd.is_none_or(|cwd| row.session.cwd == cwd))
            .map(|row| row.session.session_id.clone())
            .collect()
    }

    pub fn is_deleted(&self, id: &SessionId) -> bool {
        self.deleted.contains(id)
    }
}

pub fn project_body(
    id: ElementId,
    expanded: bool,
    window: &mut Window,
    cx: &mut Context<SessionView>,
    render: impl FnOnce(&mut Context<SessionView>) -> AnyElement,
) -> AnyElement {
    let tokens = cx.theme().motion_tokens();
    let transition = Transition::new(tokens.duration_fast).easing(tokens.easing_enter.clone());
    let expanded_before = window.use_keyed_state(
        ElementId::NamedChild(id.clone().into(), "expanded-before".into()),
        cx,
        |_, _| false,
    );
    let animate = expanded || *expanded_before.read(cx);
    if expanded && !*expanded_before.read(cx) {
        expanded_before.update(cx, |expanded, _| *expanded = true);
    }
    let presence = Presence::new((id, "body"), expanded)
        .transition(if animate { transition } else { Transition::new(Duration::ZERO) })
        .sample(window, cx);
    div()
        .when(presence.should_render(), |body| body.opacity(presence.progress).child(render(cx)))
        .into_any_element()
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
        let now = cx.background_executor().now();
        let opacity = if cx.reduce_motion() {
            if self.row.is_exiting() { 0. } else { 1. }
        } else {
            self.row.opacity(now, tokens.duration_fast)
        };
        let active =
            self.row.exit.is_some_and(|(start, duration, _)| {
                now.saturating_duration_since(start) < duration
            }) || self
                .row
                .entered_at
                .is_some_and(|start| now.saturating_duration_since(start) < tokens.duration_fast);
        if active && !cx.reduce_motion() {
            window.request_animation_frame();
        }
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
            .when(!self.row.is_exiting(), |row| {
                row.hover(|row| row.bg(if self.selected { background } else { cx.theme().accent }))
            })
            .opacity(opacity)
            .when(
                self.row.entered_at.is_some() && !self.row.is_exiting() && !cx.reduce_motion(),
                |row| row.relative().left(rems(0.25 * (1. - opacity))),
            )
            .child(self.child)
    }
}
