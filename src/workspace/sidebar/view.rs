use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{ActiveTheme, Disableable, Icon};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::NewSession;
use crate::workspace::session::model::SessionList;
use crate::workspace::settings::SettingsButton;
use crate::workspace::settings::palette::Palette;
use crate::workspace::sidebar::list::SessionListView;
use crate::workspace::sidebar::motion::SidebarMotion;
use crate::workspace::{SessionView, WorkspaceView};

pub struct Sidebar {
    list: SessionList,
    motion: SidebarMotion,
    scroll: ScrollHandle,
    anchor: ScrollAnchor,
    loading_session: Option<SessionId>,
    created_session: Option<SessionInfo>,
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
        div()
            .id("sidebar")
            .w_full()
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(p.sidebar())
            .child(
                div().p_3().child(
                    Button::new("new-chat")
                        .label("新建会话")
                        .tooltip_with_action("新建会话", &NewSession, None)
                        .icon(Icon::new(IconName::Plus).size(rems(0.9375)))
                        .w_full()
                        .h(rems(2.375))
                        .rounded(cx.theme().radius_lg)
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
                    .p_3()
                    .border_t_1()
                    .border_color(p.border())
                    .child(SettingsButton::new(tab_host.clone(), p)),
            )
    }

    pub fn sync_sidebar_rows(
        &mut self,
        current: Option<&SessionId>,
        cwd: &Path,
        title: String,
        cx: &mut Context<SessionView>,
    ) {
        let sessions = self.sidebar_sessions(current, cwd, title);
        self.motion.reconcile(sessions, cx.background_executor().now());
    }

    pub fn receive_sidebar_rows(
        &mut self,
        current: Option<&SessionId>,
        cwd: &Path,
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
        cwd: &Path,
        title: String,
    ) -> Vec<SessionInfo> {
        let mut sessions: Vec<_> =
            self.list.projects().flat_map(|cwd| self.list.project_sessions(cwd).cloned()).collect();
        if let Some(id) = current
            && !sessions.iter().any(|session| &session.session_id == id)
        {
            sessions.insert(0, SessionInfo::new(id.clone(), cwd.to_path_buf()).title(title));
        }
        sessions
    }

    pub fn begin_sidebar_exit(
        &mut self,
        ids: &HashSet<SessionId>,
        current: Option<&SessionId>,
        cwd: &Path,
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
        directory: &Path,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let cwd = current
            .and_then(|id| self.list.session(id))
            .map(|session| session.cwd.clone())
            .unwrap_or_else(|| directory.to_path_buf());
        self.list.expand_project(&cwd);
        let anchor = self.anchor.clone();
        window.on_next_frame(move |window, cx| {
            anchor.scroll_to(window, cx);
            window.on_next_frame(|window, _| window.refresh());
        });
        cx.notify();
    }
}
