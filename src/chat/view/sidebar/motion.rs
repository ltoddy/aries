use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::base::motion::{self, Presence, Transition};
use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;

#[derive(Clone)]
pub(crate) struct SidebarRow {
    pub(crate) session: SessionInfo,
    entered_at: Option<Instant>,
    exit: Option<(Instant, Duration, f32)>,
    pending_created: bool,
}

impl SidebarRow {
    pub(crate) fn is_exiting(&self) -> bool {
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
pub(crate) struct SidebarMotion {
    initialized: bool,
    seen: HashSet<SessionId>,
    deleted: HashSet<SessionId>,
    rows: Vec<SidebarRow>,
}

impl SidebarMotion {
    pub(crate) fn receive(&mut self, sessions: Vec<SessionInfo>, now: Instant) {
        let confirmed: HashSet<_> =
            sessions.iter().map(|session| session.session_id.clone()).collect();
        self.reconcile(sessions, now);
        for row in &mut self.rows {
            if confirmed.contains(&row.session.session_id) {
                row.pending_created = false;
            }
        }
        self.initialized = true;
    }

    fn reconcile(&mut self, sessions: Vec<SessionInfo>, now: Instant) {
        let mut ids = HashSet::new();
        let mut rows = Vec::new();
        for session in sessions {
            let id = &session.session_id;
            if self.deleted.contains(id) || !ids.insert(id.clone()) {
                continue;
            }
            let previous = self.rows.iter().find(|row| &row.session.session_id == id);
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

    pub(crate) fn add_created(&mut self, session: SessionInfo, now: Instant) {
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

    pub(crate) fn delete(&mut self, ids: &HashSet<SessionId>, now: Instant, duration: Duration) {
        self.deleted.extend(ids.iter().cloned());
        for row in &mut self.rows {
            if ids.contains(&row.session.session_id) && !row.is_exiting() {
                let opacity = row.opacity(now, duration);
                row.exit = Some((now, duration, opacity));
            }
        }
        self.cleanup(now, duration.is_zero());
    }

    pub(crate) fn cleanup(&mut self, now: Instant, reduced: bool) {
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

    pub(crate) fn projects(&self) -> BTreeSet<PathBuf> {
        self.rows.iter().map(|row| row.session.cwd.clone()).collect()
    }

    pub(crate) fn project_rows(&self, cwd: &Path) -> Vec<&SidebarRow> {
        self.rows.iter().filter(|row| row.session.cwd == cwd).collect()
    }

    pub(crate) fn session_ids(&self, cwd: Option<&Path>) -> Vec<SessionId> {
        self.rows
            .iter()
            .filter(|row| !row.is_exiting() && cwd.is_none_or(|cwd| row.session.cwd == cwd))
            .map(|row| row.session.session_id.clone())
            .collect()
    }

    pub(crate) fn is_deleted(&self, id: &SessionId) -> bool {
        self.deleted.contains(id)
    }
}

impl ChatView {
    pub(crate) fn sync_sidebar_rows(&mut self, cx: &mut Context<Self>) {
        let mut sessions: Vec<_> = self
            .session_list
            .projects()
            .flat_map(|cwd| self.session_list.project_sessions(cwd).cloned())
            .collect();
        if let Some(id) = &self.session_id
            && !sessions.iter().any(|session| &session.session_id == id)
        {
            sessions
                .insert(0, SessionInfo::new(id.clone(), self.cwd.clone()).title(self.chat.title()));
        }
        self.sidebar_motion.reconcile(sessions, cx.background_executor().now());
    }

    pub(crate) fn begin_sidebar_exit(&mut self, ids: &HashSet<SessionId>, cx: &mut Context<Self>) {
        self.sync_sidebar_rows(cx);
        let duration = if cx.reduce_motion() {
            Duration::ZERO
        } else {
            cx.theme().motion_tokens().duration_fast
        };
        self.sidebar_motion.delete(ids, cx.background_executor().now(), duration);
        if !duration.is_zero() && !ids.is_empty() {
            cx.spawn(async move |view, cx| {
                cx.background_executor().timer(duration).await;
                let _ = view.update(cx, |this, cx| {
                    this.sidebar_motion.cleanup(cx.background_executor().now(), cx.reduce_motion());
                    cx.notify();
                });
            })
            .detach();
        }
    }
}

#[derive(IntoElement)]
pub(crate) struct ProjectBody {
    pub(crate) id: ElementId,
    pub(crate) expanded: bool,
    pub(crate) child: AnyElement,
}

impl RenderOnce for ProjectBody {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let tokens = cx.theme().motion_tokens();
        let transition = Transition::new(tokens.duration_fast).easing(tokens.easing_enter.clone());
        let expanded_before = window.use_keyed_state(
            ElementId::NamedChild(self.id.clone().into(), "expanded-before".into()),
            cx,
            |_, _| false,
        );
        let animate = self.expanded || *expanded_before.read(cx);
        if self.expanded && !*expanded_before.read(cx) {
            expanded_before.update(cx, |expanded, _| *expanded = true);
        }
        let presence = Presence::new((self.id, "body"), self.expanded)
            .transition(if animate { transition } else { Transition::new(Duration::ZERO) })
            .sample(window, cx);
        div().when(presence.should_render(), |body| {
            body.opacity(presence.progress).child(self.child)
        })
    }
}

#[derive(IntoElement)]
pub(crate) struct SessionSurface {
    pub(crate) row: SidebarRow,
    pub(crate) selected: bool,
    pub(crate) background: Hsla,
    pub(crate) child: AnyElement,
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
            .opacity(opacity)
            .when(
                self.row.entered_at.is_some() && !self.row.is_exiting() && !cx.reduce_motion(),
                |row| row.relative().left(rems(0.25 * (1. - opacity))),
            )
            .child(self.child)
    }
}
