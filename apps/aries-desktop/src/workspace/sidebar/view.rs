use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::session::model::SessionList;
use crate::workspace::settings::SettingsButton;
use crate::workspace::settings::palette::Palette;
use crate::workspace::sidebar::list::SessionListView;
use crate::workspace::sidebar::motion::SidebarMotion;
use crate::workspace::{SessionView, WorkspaceView};

pub const SESSION_LIMIT: usize = 8;

pub struct Sidebar {
    list: SessionList,
    motion: SidebarMotion,
    scroll: ScrollHandle,
    anchor: ScrollAnchor,
    loading_session: Option<SessionId>,
    created_session: Option<SessionInfo>,
    expanded_sessions: HashSet<PathBuf>,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self::new()
    }
}

impl Sidebar {
    pub fn new() -> Self {
        let scroll = ScrollHandle::new();
        let anchor = ScrollAnchor::for_handle(scroll.clone());
        Self {
            list: SessionList::default(),
            motion: SidebarMotion::default(),
            scroll,
            anchor,
            loading_session: None,
            created_session: None,
            expanded_sessions: HashSet::new(),
        }
    }
    pub fn scroll(&self) -> &ScrollHandle {
        &self.scroll
    }
    pub fn anchor(&self) -> &ScrollAnchor {
        &self.anchor
    }
    pub fn list(&self) -> &SessionList {
        &self.list
    }
    pub fn list_mut(&mut self) -> &mut SessionList {
        &mut self.list
    }
    pub fn motion(&self) -> &SidebarMotion {
        &self.motion
    }
    pub fn motion_mut(&mut self) -> &mut SidebarMotion {
        &mut self.motion
    }
    pub fn loading_session(&self) -> Option<&SessionId> {
        self.loading_session.as_ref()
    }
    pub fn set_loading_session(&mut self, session: Option<SessionId>) {
        self.loading_session = session;
    }
    pub fn set_created_session(&mut self, session: SessionInfo) {
        self.created_session = Some(session);
    }
    pub fn take_created_session(&mut self) -> Option<SessionInfo> {
        self.created_session.take()
    }
    pub fn is_sessions_expanded(&self, cwd: impl AsRef<Path>) -> bool {
        self.expanded_sessions.contains(cwd.as_ref())
    }
    pub fn toggle_sessions(&mut self, cwd: impl AsRef<Path>) {
        let cwd = cwd.as_ref();
        if !self.expanded_sessions.remove(cwd) {
            self.expanded_sessions.insert(cwd.to_owned());
        }
    }
    pub fn collapse_all(&mut self) {
        self.list.collapse_all();
        self.expanded_sessions.clear();
    }
    pub fn cleanup(&mut self, cx: &App) {
        self.motion.cleanup(cx.background_executor().now(), cx.reduce_motion());
    }

    pub fn render(
        &self,
        owner: &SessionView,
        tab_host: Option<WeakEntity<WorkspaceView>>,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) -> impl IntoElement {
        let new_enabled = tab_host
            .as_ref()
            .and_then(|host| host.upgrade())
            .and_then(|host| {
                host.read(cx).active_view().map(|view| view.read(cx).can_start_new_chat())
            })
            .unwrap_or_else(|| owner.can_start_new_chat());
        let usage_selected = tab_host
            .as_ref()
            .and_then(|host| host.upgrade())
            .is_some_and(|host| host.read(cx).is_usage_visible());
        let usage_host = tab_host.clone();
        div()
            .id("sidebar")
            .w_full()
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(p.sidebar().opacity(0.))
            .child(
                div().px_2().py_1().child(
                    Button::new("new-chat")
                        .ghost()
                        .small()
                        .label("新建会话…")
                        .tooltip("选择项目目录并新建会话")
                        .icon(Icon::new(IconName::Plus).size(rems(0.9375)))
                        .w_full()
                        .text_color(p.muted())
                        .rounded(cx.theme().radius)
                        .disabled(!new_enabled)
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.uses_sidebar_tab_host() {
                                this.request_new_tab(cx);
                            } else {
                                this.new_chat(window, cx);
                            }
                        })),
                ),
            )
            .child(SessionListView::new(self, owner).render_sessions(p, window, cx))
            .child(
                div()
                    .flex_shrink_0()
                    .px_2()
                    .py_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(SettingsButton::new(tab_host.clone(), p))
                    .child(
                        Button::new("open-usage")
                            .ghost()
                            .small()
                            .icon(Icon::new(IconName::ChartNoAxesColumn).size(rems(0.9375)))
                            .tooltip("统计")
                            .accessibility_label("打开统计")
                            .selected(usage_selected)
                            .bg(if usage_selected {
                                cx.theme().sidebar_accent
                            } else {
                                cx.theme().transparent
                            })
                            .text_color(if usage_selected { p.foreground() } else { p.muted() })
                            .disabled(usage_host.is_none())
                            .on_click(cx.listener(move |_, _, window, cx| {
                                if let Some(host) = usage_host.clone() {
                                    cx.spawn_in(window, async move |_, cx| {
                                        let _ = host.update_in(cx, |host, window, cx| {
                                            host.open_usage(window, cx)
                                        });
                                    })
                                    .detach();
                                }
                            })),
                    ),
            )
    }

    pub fn sync_sidebar_rows(
        &mut self,
        current: Option<&SessionId>,
        cwd: impl AsRef<Path>,
        title: String,
        cx: &mut Context<SessionView>,
    ) {
        let sessions = self.sidebar_sessions(current, cwd, title);
        self.motion.reconcile(sessions, cx.background_executor().now());
    }

    pub fn receive_sidebar_rows(
        &mut self,
        current: Option<&SessionId>,
        cwd: impl AsRef<Path>,
        title: String,
        cx: &mut Context<SessionView>,
    ) {
        let confirmed: HashSet<_> =
            self.list.entries().iter().map(|session| session.session_id.clone()).collect();
        let sessions = self.sidebar_sessions(current, cwd, title);
        self.motion.receive(sessions, &confirmed, cx.background_executor().now());
    }

    fn sidebar_sessions(
        &self,
        current: Option<&SessionId>,
        cwd: impl AsRef<Path>,
        title: String,
    ) -> Vec<SessionInfo> {
        let mut sessions: Vec<_> =
            self.list.projects().flat_map(|cwd| self.list.project_sessions(cwd).cloned()).collect();
        if let Some(id) = current
            && !sessions.iter().any(|session| &session.session_id == id)
        {
            sessions.insert(0, SessionInfo::new(id.clone(), cwd.as_ref().to_owned()).title(title));
        }
        sessions
    }

    pub fn begin_sidebar_exit(
        &mut self,
        ids: &HashSet<SessionId>,
        current: Option<&SessionId>,
        cwd: impl AsRef<Path>,
        title: String,
        cx: &mut Context<SessionView>,
    ) {
        self.sync_sidebar_rows(current, cwd, title, cx);
        let duration = if cx.reduce_motion() {
            Duration::ZERO
        } else {
            cx.theme().motion_tokens().duration_fast
        };
        self.motion.delete(ids, cx.background_executor().now(), duration);
        if !duration.is_zero() && !ids.is_empty() {
            cx.spawn(async move |view, cx| {
                cx.background_executor().timer(duration).await;
                let _ = view.update(cx, |this, cx| {
                    this.with_sidebar(|sidebar| sidebar.cleanup(cx));
                    cx.notify();
                });
            })
            .detach();
        }
    }
    pub fn locate_current_session(
        &mut self,
        current: Option<&SessionId>,
        directory: impl AsRef<Path>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let cwd = current
            .and_then(|id| self.list.session(id))
            .map(|session| session.cwd.clone())
            .unwrap_or_else(|| directory.as_ref().to_owned());
        self.list.expand_project(&cwd);
        if let Some(id) = current
            && self.motion.projects().get(cwd.as_path()).is_some_and(|sessions| {
                sessions.iter().skip(SESSION_LIMIT).any(|row| &row.session().session_id == id)
            })
        {
            self.expanded_sessions.insert(cwd);
        }
        let anchor = self.anchor.clone();
        window.on_next_frame(move |window, cx| {
            anchor.scroll_to(window, cx);
            window.on_next_frame(|window, _| window.refresh());
        });
        cx.notify();
    }
}
