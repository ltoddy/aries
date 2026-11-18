use crate::workspace::workspace_view::*;

impl WorkspaceView {
    pub fn selected_session_view(&self) -> Option<&Entity<SessionView>> {
        self.tabs.get(self.active).and_then(WorkspaceTab::session)
    }

    pub fn selected_file_view(&self) -> Option<&Entity<FileView>> {
        self.tabs.get(self.active).and_then(WorkspaceTab::file)
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

    pub fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.selected_session_view() {
            view.update(cx, |view, cx| view.new_chat_in_current_directory(window, cx));
        } else {
            let directory = self.current_directory(cx);
            self.sidebar.update(cx, |view, cx| view.new_chat_in_directory(directory, window, cx));
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
        if !self.pending_file_operations.is_empty() {
            return;
        }
        if !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx)) {
            return;
        }
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
        if !self.is_sessions_visible() {
            self.show_sessions(window, cx);
        }
        let id = self.next_id;
        self.next_id += 1;
        let view =
            Self::create_chat_view(&directory, &self.agent, SessionRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        self.tabs.push(WorkspaceTab::new(
            id,
            WorkspaceTabKind::session(SessionTab::new(id, view.clone(), subscription)),
        ));
        self.file_tree.update(cx, |tree, cx| tree.set_directory(directory.clone(), cx));
        self.page = WorkspacePage::sessions();
        self.active = self.tabs.len() - 1;
        self.tab_scroll.scroll_to_item(self.active);
        self.sync_sidebar_active_session(cx);
        self.focus_active_tab(window, cx);
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
            || self.sidebar.read(cx).management().is_pending()
            || !self.pending_file_operations.is_empty()
        {
            return;
        }
        if !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx)) {
            return;
        }
        if !self.is_sessions_visible() {
            self.show_sessions(window, cx);
        }
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
        let session_id = session.session_id.clone();
        if let Some(index) =
            self.archived_tabs.iter().position(|tab| tab.view.read(cx).matches_session(&session_id))
        {
            let tab = self.archived_tabs.remove(index);
            tab.view.update(cx, |view, cx| view.reload_archived_session(session, window, cx));
            self.tabs.push(WorkspaceTab::new(tab.id, WorkspaceTabKind::session(tab)));
            self.select_tab(self.tabs.len() - 1, window, cx);
            return;
        }
        if let Some(index) = self.find_session_tab(&session_id, cx) {
            self.select_tab(index, window, cx);
            self.set_sidebar_session(session_id, cx);
            cx.notify();
            return;
        }
        if let Some(view) = self.active_view()
            && view.read(cx).is_empty_tab(cx)
        {
            view.update(cx, |view, cx| view.queue_session_load(session, cx));
            self.focus_active_tab(window, cx);
            self.set_sidebar_session(session_id, cx);
            cx.notify();
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let view =
            Self::create_chat_view(&session.cwd, &self.agent, SessionRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        view.update(cx, |view, cx| view.queue_session_load(session, cx));
        self.tabs.push(WorkspaceTab::new(
            id,
            WorkspaceTabKind::session(SessionTab::new(id, view, subscription)),
        ));
        self.page = WorkspacePage::sessions();
        self.active = self.tabs.len() - 1;
        self.tab_scroll.scroll_to_item(self.active);
        self.set_sidebar_session(session_id, cx);
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    pub fn find_session_tab(&self, id: &SessionId, cx: &App) -> Option<usize> {
        self.tabs
            .iter()
            .position(|tab| tab.session().is_some_and(|view| view.read(cx).matches_session(id)))
    }

    pub fn session_management_source(
        &self,
        id: &SessionId,
        cx: &App,
    ) -> Option<&Entity<SessionView>> {
        self.tabs
            .iter()
            .filter_map(WorkspaceTab::session)
            .chain(self.archived_tabs.iter().map(|tab| &tab.view))
            .find(|view| view.read(cx).matches_session(id))
    }

    pub fn forward_session_management_event(
        &mut self,
        event: Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sidebar = self.sidebar.clone();
        window.defer(cx, move |window, cx| {
            sidebar.update(cx, |sidebar, cx| sidebar.apply_event(event, window, cx));
        });
    }

    pub fn fail_session_management(&mut self, id: &SessionId, cx: &mut Context<Self>) {
        let sidebar = self.sidebar.clone();
        let id = id.clone();
        cx.defer(move |cx| {
            sidebar.update(cx, |sidebar, cx| sidebar.fail_session_management(&id, cx));
        });
    }

    pub fn session_management_available(&self, id: &SessionId, cx: &App) -> bool {
        self.tabs
            .iter()
            .filter_map(WorkspaceTab::session)
            .chain(self.archived_tabs.iter().map(|tab| &tab.view))
            .filter(|view| view.read(cx).matches_session(id))
            .all(|view| {
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
        for view in self
            .tabs
            .iter()
            .filter_map(WorkspaceTab::session)
            .chain(self.archived_tabs.iter().map(|tab| &tab.view))
        {
            if view.read(cx).matches_session(id) {
                view.update(cx, |view, cx| view.set_management_lock(locked, cx));
            }
        }
    }
    pub fn sync_session_title(&mut self, id: &SessionId, title: String, cx: &mut Context<Self>) {
        for view in self
            .tabs
            .iter()
            .filter_map(WorkspaceTab::session)
            .chain(self.archived_tabs.iter().map(|tab| &tab.view))
        {
            view.update(cx, |view, cx| view.sync_session_title(id, title.clone(), cx));
        }
        self.sync_sidebar_active_session(cx);
        cx.notify();
    }
    pub fn close_archived_session(&mut self, id: &SessionId, cx: &mut Context<Self>) {
        let active_id = self.tabs.get(self.active).map(|tab| tab.id);
        let mut index = 0;
        while index < self.tabs.len() {
            if self.tabs[index].session().is_some_and(|view| view.read(cx).matches_session(id)) {
                let tab = self.tabs.remove(index);
                if let WorkspaceTabKind::Session(tab) = tab.kind {
                    self.archived_tabs.push(tab);
                }
            } else {
                index += 1;
            }
        }
        self.active = active_id
            .and_then(|id| self.tabs.iter().position(|tab| tab.id == id))
            .unwrap_or_else(|| self.active.min(self.tabs.len().saturating_sub(1)));
        let changed_active = active_id != self.tabs.get(self.active).map(|tab| tab.id);
        if changed_active {
            self.invalidate_file_navigation(cx);
        }
        self.tab_scroll.scroll_to_item(self.active);
        self.sync_sidebar_active_session(cx);
        if changed_active {
            self.defer_active_tab_focus(cx);
        }
        cx.notify();
    }
    pub fn close_deleted_sessions(&mut self, deleted: &HashSet<SessionId>, cx: &mut Context<Self>) {
        self.archived_tabs
            .retain(|tab| !deleted.iter().any(|id| tab.view.read(cx).matches_session(id)));
        let ids: Vec<_> = self
            .tabs
            .iter()
            .filter(|tab| {
                tab.session()
                    .is_some_and(|view| deleted.iter().any(|id| view.read(cx).matches_session(id)))
            })
            .map(|tab| tab.id)
            .collect();
        for id in ids {
            self.close_tab(id, cx);
        }
    }
}
