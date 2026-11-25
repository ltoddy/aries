use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::settings::{Preferences, SettingsState};
use crate::workspace::workspace_view::{WorkspacePage, WorkspaceTab, WorkspaceView};

impl WorkspaceView {
    pub fn preferences<'a>(&'a self, cx: &'a App) -> &'a Preferences {
        self.settings.read(cx).preferences()
    }

    pub fn settings_state(&self) -> &Entity<SettingsState> {
        &self.settings
    }

    pub fn active_page(&self) -> Option<WorkspacePage> {
        self.tabs.active_tab().and_then(WorkspaceTab::page)
    }

    pub fn is_sessions_visible(&self) -> bool {
        self.active_page().is_none()
    }

    pub fn open_page(&mut self, page: WorkspacePage, window: &mut Window, cx: &mut Context<Self>) {
        match page {
            WorkspacePage::Usage => self.usage.update(cx, |view, cx| view.open(window, cx)),
            WorkspacePage::Extensions => {
                let directory = self.current_directory(cx);
                self.extensions.update(cx, |view, cx| view.open(directory, window, cx));
            },
            WorkspacePage::Settings => {},
        }
        let index = match self.tabs.items.iter().position(|tab| tab.page() == Some(page)) {
            Some(index) => index,
            None => {
                let id = self.tabs.alloc_id();
                self.tabs.items.push(WorkspaceTab::new_page(id, page));
                self.tabs.items.len() - 1
            },
        };
        self.try_activate(index, window, cx);
    }

    pub fn open_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_page(WorkspacePage::usage(), window, cx);
    }

    pub fn open_extensions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.open_page(WorkspacePage::extensions(), window, cx);
    }

    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_navigate(window, cx) {
            return;
        }
        self.open_page(WorkspacePage::settings(), window, cx);
    }

    pub fn notify_settings_sidebar(&self, cx: &mut Context<Self>) {
        self.sidebar_view.update(cx, |_, cx| cx.notify());
    }

    pub fn notify_settings_tabs(&self, cx: &mut Context<Self>) {
        for tab in &self.tabs.items {
            tab.notify(cx);
        }
    }

    pub fn remeasure_timelines(&self, cx: &mut Context<Self>) {
        self.sidebar_view.read(cx).remeasure_timeline();
        for view in self.tabs.items.iter().filter_map(WorkspaceTab::session) {
            view.read(cx).remeasure_timeline();
        }
        cx.notify();
    }
}
