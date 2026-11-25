use std::collections::HashSet;
use std::path::{Path, PathBuf};

use agent_client_protocol::AcpAgentConfig;
use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::acp::Event;
use crate::workspace::files::FileView;
use crate::workspace::workspace_view::{Tab, WorkspaceTab, WorkspaceView};
use crate::workspace::{SessionRenderMode, SessionView};

impl WorkspaceView {
    pub fn create_chat_view(
        cwd: impl AsRef<Path>,
        agent: &AcpAgentConfig,
        mode: SessionRenderMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SessionView> {
        let host = cx.weak_entity();
        let cwd = cwd.as_ref().to_owned();
        let agent = agent.clone();
        let view = cx.new(|cx| SessionView::new(cwd, agent, window, cx));
        view.update(cx, |view, _| {
            view.set_tab_host(host);
            view.set_render_mode(mode);
        });
        view
    }

    pub fn observe_tab(
        view: &Entity<SessionView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.observe_in(view, window, |this, view, window, cx| {
            if view.update(cx, |view, _| view.take_new_tab_request()) {
                if this.selected_file_view().is_some() {
                    this.new_session(window, cx);
                } else {
                    this.active_view().unwrap_or(&this.sidebar_view).update(cx, |view, cx| {
                        view.new_chat(window, cx);
                    });
                }
            } else if let Some(session) = view.update(cx, |view, _| view.take_session_tab_request())
            {
                this.open_session(session, window, cx);
            } else {
                if view != this.sidebar_view {
                    let created = view.update(cx, |view, _| view.take_created_sidebar_session());
                    if let Some(session) = created {
                        this.sidebar_view.update(cx, |sidebar, cx| {
                            sidebar.receive_created_sidebar_session(session, cx);
                        });
                    }
                    if this.active_view() == Some(&view) {
                        this.sync_sidebar_active_session(cx);
                    }
                }
                cx.notify();
            }
        })
    }

    pub fn sync_sidebar_active_session(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.active_view().map(|view| view.read(cx).sidebar_snapshot());
        self.sidebar_view.update(cx, |view, cx| view.sync_sidebar_active(snapshot, cx));
        self.sync_file_directory(cx);
    }

    pub fn set_sidebar_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        self.sidebar_view.update(cx, |view, cx| view.set_sidebar_session(session_id, cx));
        self.sync_file_directory(cx);
    }

    pub fn selected_session_view(&self) -> Option<&Entity<SessionView>> {
        let tab = self.tabs.active_tab()?;
        tab.session().or_else(|| tab.diff_session())
    }

    pub fn selected_file_view(&self) -> Option<&Entity<FileView>> {
        self.tabs.active_tab().and_then(WorkspaceTab::file)
    }

    pub fn active_view(&self) -> Option<&Entity<SessionView>> {
        self.is_sessions_visible().then(|| self.selected_session_view()).flatten()
    }

    pub fn active_file_view(&self) -> Option<&Entity<FileView>> {
        self.is_sessions_visible().then(|| self.selected_file_view()).flatten()
    }

    pub fn current_directory(&self, cx: &App) -> PathBuf {
        self.selected_file_directory(cx).unwrap_or_else(|| self.cwd.clone())
    }

    pub fn session_views(&self) -> impl Iterator<Item = &Entity<SessionView>> {
        self.tabs
            .items
            .iter()
            .filter_map(WorkspaceTab::session)
            .chain(self.tabs.archived.iter().map(Tab::view))
    }

    pub fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.selected_session_view() {
            view.update(cx, |view, cx| view.new_chat_in_current_directory(window, cx));
        } else {
            let directory = self.current_directory(cx);
            self.sidebar_view
                .update(cx, |view, cx| view.new_chat_in_directory(directory, window, cx));
        }
    }

    pub fn new_tab_in_directory(
        &mut self,
        directory: impl Into<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.create_tab(directory.into(), window, cx);
    }

    pub fn create_tab(&mut self, directory: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.files.pending.is_empty() {
            return;
        }
        if !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx)) {
            return;
        }
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
        let id = self.tabs.alloc_id();
        let view =
            Self::create_chat_view(&directory, &self.agent, SessionRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        self.tabs.items.push(WorkspaceTab::new_session(id, view.clone(), subscription));
        self.file_tree.update(cx, |tree, cx| tree.set_directory(directory.clone(), cx));
        self.activate(self.tabs.items.len() - 1, window, cx);
        let view = view.downgrade();
        cx.spawn_in(window, async move |_, cx| {
            let _ = cx.update(|window, cx| {
                let _ = view.update(cx, |view, cx| view.start_new_chat(directory, window, cx));
            });
        })
        .detach();
        cx.notify();
    }

    pub fn open_session(
        &mut self,
        session: SessionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if session.archived.unwrap_or(false)
            || self.sidebar_view.read(cx).management().is_pending()
            || self.files.pending.is_empty()
        {
            return;
        }
        if !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx)) {
            return;
        }
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
        let session_id = session.session_id.clone();
        if let Some(index) = self
            .tabs
            .archived
            .iter()
            .position(|tab| tab.view().read(cx).matches_session(&session_id))
        {
            let tab = self.tabs.archived.remove(index);
            tab.view().update(cx, |view, cx| view.reload_archived_session(session, window, cx));
            self.tabs.items.push(WorkspaceTab::resumed_session(tab));
            self.try_activate(self.tabs.items.len() - 1, window, cx);
            return;
        }
        if let Some(index) = self.find_session_tab(&session_id, cx) {
            self.try_activate(index, window, cx);
            self.set_sidebar_session(session_id, cx);
            cx.notify();
            return;
        }
        if let Some(view) = self.active_view()
            && view.read(cx).is_empty_tab(cx)
        {
            view.update(cx, |view, cx| view.queue_session_load(session, cx));
            self.focus_active(window, cx);
            self.set_sidebar_session(session_id, cx);
            cx.notify();
            return;
        }
        let id = self.tabs.alloc_id();
        let view =
            Self::create_chat_view(&session.cwd, &self.agent, SessionRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        view.update(cx, |view, cx| view.queue_session_load(session, cx));
        self.tabs.items.push(WorkspaceTab::new_session(id, view, subscription));
        self.tabs.active = self.tabs.items.len() - 1;
        self.tabs.scroll.scroll_to_item(self.tabs.active);
        self.set_sidebar_session(session_id, cx);
        self.focus_active(window, cx);
        cx.notify();
    }

    pub fn find_session_tab(&self, id: &SessionId, cx: &App) -> Option<usize> {
        self.tabs
            .items
            .iter()
            .position(|tab| tab.session().is_some_and(|view| view.read(cx).matches_session(id)))
    }

    pub fn session_management_source(
        &self,
        id: &SessionId,
        cx: &App,
    ) -> Option<&Entity<SessionView>> {
        self.session_views().find(|view| view.read(cx).matches_session(id))
    }

    pub fn forward_session_management_event(
        &mut self,
        event: Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sidebar = self.sidebar_view.clone();
        window.defer(cx, move |window, cx| {
            sidebar.update(cx, |sidebar, cx| sidebar.apply_event(event, window, cx));
        });
    }

    pub fn fail_session_management(&mut self, id: &SessionId, cx: &mut Context<Self>) {
        let sidebar = self.sidebar_view.clone();
        let id = id.clone();
        cx.defer(move |cx| {
            sidebar.update(cx, |sidebar, cx| sidebar.fail_session_management(&id, cx));
        });
    }

    pub fn session_management_available(&self, id: &SessionId, cx: &App) -> bool {
        self.session_views().filter(|view| view.read(cx).matches_session(id)).all(|view| {
            let view = view.read(cx);
            view.is_session_idle() && !view.execution().is_disconnected()
        })
    }
    pub fn lock_session_management(
        &mut self,
        id: &SessionId,
        locked: bool,
        cx: &mut Context<Self>,
    ) {
        for view in self.session_views() {
            if view.read(cx).matches_session(id) {
                view.update(cx, |view, cx| view.set_management_lock(locked, cx));
            }
        }
    }
    pub fn sync_session_title(&mut self, id: &SessionId, title: String, cx: &mut Context<Self>) {
        for view in self.session_views() {
            view.update(cx, |view, cx| view.sync_session_title(id, title.clone(), cx));
        }
        self.sync_sidebar_active_session(cx);
        cx.notify();
    }
    pub fn close_archived_session(&mut self, id: &SessionId, cx: &mut Context<Self>) {
        let active_id = self.active_id();
        let mut index = 0;
        while index < self.tabs.items.len() {
            if self.tabs.tab(index).is_some_and(|tab| {
                tab.session().is_some_and(|view| view.read(cx).matches_session(id))
            }) {
                let tab = self.tabs.items.remove(index);
                if let WorkspaceTab::Session(tab) = tab {
                    self.tabs.archived.push(tab);
                }
            } else {
                index += 1;
            }
        }
        self.tabs
            .items
            .retain(|tab| tab.diff_session().is_none_or(|view| !view.read(cx).matches_session(id)));
        self.tabs.active = active_id
            .and_then(|id| self.tab_index(id))
            .unwrap_or_else(|| self.tabs.active.min(self.tabs.items.len().saturating_sub(1)));
        let changed_active = active_id != self.active_id();
        if changed_active {
            self.invalidate_file_navigation(cx);
        }
        self.tabs.scroll.scroll_to_item(self.tabs.active);
        self.sync_sidebar_active_session(cx);
        if changed_active {
            self.defer_active_tab_focus(cx);
        }
        cx.notify();
    }
    pub fn close_deleted_sessions(&mut self, deleted: &HashSet<SessionId>, cx: &mut Context<Self>) {
        self.tabs
            .archived
            .retain(|tab| !deleted.iter().any(|id| tab.view().read(cx).matches_session(id)));
        let ids: Vec<_> = self
            .tabs
            .items
            .iter()
            .filter(|tab| {
                tab.session()
                    .is_some_and(|view| deleted.iter().any(|id| view.read(cx).matches_session(id)))
            })
            .map(WorkspaceTab::id)
            .collect();
        for id in ids {
            self.close_tab(id, cx);
        }
    }
}
