use std::path::PathBuf;

use gpui_kit::component::ActiveTheme;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::workspace_view::{LayoutSizes, SidebarPanel, WorkspaceView};

pub struct SidebarState {
    pub panels: Vec<SidebarPanel>,
    pub panel: SidebarPanel,
    pub visible: bool,
    pub focus_handle: FocusHandle,
    pub toggle_focus_handle: FocusHandle,
    pub sessions_focus_handle: FocusHandle,
}

impl SidebarState {
    pub fn new(cx: &mut Context<WorkspaceView>) -> Self {
        Self {
            panels: vec![SidebarPanel::sessions(), SidebarPanel::files(), SidebarPanel::git()],
            panel: SidebarPanel::sessions(),
            visible: true,
            focus_handle: cx.focus_handle(),
            toggle_focus_handle: cx.focus_handle().tab_stop(true),
            sessions_focus_handle: cx.focus_handle().tab_stop(true),
        }
    }

    pub fn toggle_visible(&mut self) -> bool {
        self.visible = !self.visible;
        self.visible
    }
}

impl WorkspaceView {
    pub fn select_sidebar_panel(
        &mut self,
        panel: SidebarPanel,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let panel = if panel.is_available(self, cx) { panel } else { SidebarPanel::sessions() };
        self.sidebar.panel = panel;
        self.sidebar.visible = true;
        panel.prepare(self, cx);
        let view = cx.weak_entity();
        let revision = self.file_navigation_revision;
        let active_id = self.active_id();
        window.on_next_frame(move |window, cx| {
            let _ = view.update(cx, |this, cx| {
                if this.sidebar.panel != panel
                    || !this.sidebar.visible
                    || this.file_navigation_revision != revision
                    || this.active_id() != active_id
                {
                    return;
                }
                let minimum = LayoutSizes::new(cx.theme().font_size, window.viewport_size())
                    .sidebar_focus_window();
                if window.viewport_size().width < minimum {
                    this.sidebar.toggle_focus_handle.focus(window, cx);
                } else {
                    panel.focus(this, window, cx);
                }
            });
        });
        cx.notify();
    }

    pub fn selected_file_directory(&self, cx: &App) -> Option<PathBuf> {
        if let Some(target) = self.selected_file_view().and_then(|view| view.read(cx).target()) {
            return Some(target.directory().to_owned());
        }
        let execution = self.selected_session_view()?.read(cx).execution();
        (execution.active_session_id().is_some() || execution.directory_selected())
            .then(|| execution.tab_directory().to_owned())
            .filter(|directory| !directory.as_os_str().is_empty())
    }

    pub fn sync_file_directory(&mut self, cx: &mut Context<Self>) {
        let Some(directory) = self.selected_file_directory(cx) else {
            self.file_tree.update(cx, |tree, cx| tree.track_current_file(None, cx));
            if self.sidebar.panel == SidebarPanel::files() {
                self.sidebar.panel = SidebarPanel::sessions();
                let owner = cx.weak_entity();
                let window = self.window;
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if this.selected_file_directory(cx).is_none() && this.sidebar.visible {
                                this.sidebar.sessions_focus_handle.focus(window, cx);
                            }
                        });
                    });
                });
                cx.notify();
            }
            return;
        };
        let current = self
            .active_file_view()
            .and_then(|view| view.read(cx).target())
            .map(|target| target.path().to_owned());
        self.file_tree.update(cx, |tree, cx| {
            tree.set_directory(directory, cx);
            tree.track_current_file(current, cx);
        });
    }

    pub fn active_sidebar_panel(&self, cx: &App) -> SidebarPanel {
        if self.sidebar.panel.is_available(self, cx) {
            self.sidebar.panel
        } else {
            SidebarPanel::sessions()
        }
    }

    pub fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar.toggle_visible();
        if !self.sidebar.visible && self.sidebar.focus_handle.contains_focused(window, cx) {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }
}
