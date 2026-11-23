use crate::workspace::workspace_view::*;

impl WorkspaceView {
    pub fn preferences<'a>(&'a self, cx: &'a App) -> &'a Preferences {
        self.settings.read(cx).preferences()
    }

    pub fn settings_state(&self) -> &Entity<SettingsState> {
        &self.settings
    }

    pub fn is_sessions_visible(&self) -> bool {
        self.page == WorkspacePage::sessions()
    }

    pub fn is_usage_visible(&self) -> bool {
        self.page == WorkspacePage::usage()
    }

    pub fn is_extensions_visible(&self) -> bool {
        self.page == WorkspacePage::extensions()
    }

    pub fn open_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = WorkspacePage::usage();
        self.invalidate_file_navigation(cx);
        self.sync_sidebar_active_session(cx);
        self.usage.update(cx, |view, cx| view.open(window, cx));
        self.sidebar.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn open_extensions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let directory = self.current_directory(cx);
        self.page = WorkspacePage::extensions();
        self.invalidate_file_navigation(cx);
        self.sync_sidebar_active_session(cx);
        self.extensions.update(cx, |view, cx| view.open(directory, window, cx));
        self.sidebar.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn show_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_sessions_visible() {
            return;
        }
        self.page = WorkspacePage::sessions();
        self.invalidate_file_navigation(cx);
        self.sync_sidebar_active_session(cx);
        self.focus_active_tab(window, cx);
        self.sidebar.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_confirmation.is_some()
            || !self.pending_file_operations.is_empty()
            || window.has_active_dialog(cx)
        {
            return;
        }
        self.settings.update(cx, |settings, cx| settings.open_settings(window, cx));
    }

    pub fn notify_settings_sidebar(&self, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |_, cx| cx.notify());
    }

    pub fn notify_settings_tabs(&self, cx: &mut Context<Self>) {
        for tab in &self.tabs {
            match &tab.kind {
                WorkspaceTabKind::Session(tab) => tab.view.update(cx, |_, cx| cx.notify()),
                WorkspaceTabKind::File(tab) => tab.view.update(cx, |_, cx| cx.notify()),
                WorkspaceTabKind::Diff(tab) => tab.view.update(cx, |_, cx| cx.notify()),
            }
        }
    }

    pub fn remeasure_timelines(&self, cx: &mut Context<Self>) {
        self.sidebar.read(cx).remeasure_timeline();
        for view in self.tabs.iter().filter_map(WorkspaceTab::session) {
            view.read(cx).remeasure_timeline();
        }
        cx.notify();
    }
}
