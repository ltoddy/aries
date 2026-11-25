use gpui_kit::component::WindowExt;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::git::diff_view::{DiffTargetChanged, GitDiffView};
use crate::workspace::workspace_view::{
    DiffTab, DragTab, PendingFileOperation, Tab, TabCommand, WorkspaceTab, WorkspaceView,
};

impl WorkspaceView {
    pub fn tab_index(&self, id: usize) -> Option<usize> {
        self.tabs.index_of(id)
    }

    pub fn active_tab(&self) -> Option<&WorkspaceTab> {
        self.tabs.active_tab()
    }

    pub fn active_id(&self) -> Option<usize> {
        self.tabs.active_id()
    }

    pub fn has_saving_file(&self, cx: &App) -> bool {
        self.tabs.items.iter().any(|tab| tab.is_saving(cx))
    }

    pub fn can_navigate(&self, window: &mut Window, cx: &mut App) -> bool {
        self.files.confirmation.is_none()
            && !self.files.pending.is_empty()
            && !window.has_active_dialog(cx)
    }

    pub fn begin_navigation(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.can_navigate(window, cx)
            || !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx))
        {
            return false;
        }
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
        true
    }

    pub fn activate(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.tabs.active = index;
        self.tabs.scroll.scroll_to_item(index);
        self.sync_sidebar_active_session(cx);
        self.focus_active(window, cx);
        cx.notify();
    }

    pub fn close_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        if self
            .file_view(id)
            .is_some_and(|view| view.read(cx).is_dirty() || view.read(cx).is_saving())
            || self.files.pending.iter().any(|operation| operation.tab_id() == Some(id))
        {
            return;
        }
        self.remove_tab(id, cx);
    }

    pub fn remove_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(index) = self.tab_index(id) else {
            return;
        };
        let Some(tab) = self.tabs.tab(index) else {
            return;
        };
        if tab.is_pending(cx) || tab.is_saving(cx) {
            return;
        }
        let active_id = self.active_id();
        let session = tab.session().map(|view| view.entity_id());
        self.tabs.items.remove(index);
        if let Some(session) = session {
            self.tabs
                .items
                .retain(|tab| tab.diff_session().is_none_or(|view| view.entity_id() != session));
        }
        self.tabs.active = active_id
            .and_then(|id| self.tab_index(id))
            .unwrap_or_else(|| index.min(self.tabs.items.len().saturating_sub(1)));
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

    pub fn tab_order(&self) -> Vec<usize> {
        self.tabs.items.iter().map(WorkspaceTab::id).collect()
    }

    pub fn move_tab(&mut self, drag: &DragTab, target: usize, cx: &mut Context<Self>) {
        if drag.owner != cx.weak_entity() || drag.id == target {
            return;
        }
        let mut order = self.tab_order();
        let Some(from) = order.iter().position(|id| *id == drag.id) else {
            return;
        };
        let Some(to) = order.iter().position(|id| *id == target) else {
            return;
        };
        let active = self.active_id();
        order.remove(from);
        order.insert(to, drag.id);
        self.tabs.items.sort_by_key(|tab| order.iter().position(|id| *id == tab.id()));
        self.tabs.active = active.and_then(|id| self.tab_index(id)).unwrap_or(0);
        self.tabs.scroll.scroll_to_item(to);
        cx.notify();
    }

    pub fn execute_tab_command(
        &mut self,
        command: TabCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let operations = self
            .tabs
            .items
            .iter()
            .filter(|tab| {
                !tab.is_pending(cx)
                    && match command {
                        TabCommand::Close(id) => tab.id() == id,
                        TabCommand::CloseAll => true,
                        TabCommand::CloseOthers(id) => tab.id() != id,
                    }
            })
            .map(|tab| PendingFileOperation::close(tab.id()))
            .collect();
        self.request_file_operations(operations, window, cx);
    }

    pub fn defer_active_tab_focus(&self, cx: &mut Context<Self>) {
        let id = self.active_id();
        let revision = self.file_navigation_revision;
        let window = self.window;
        let owner = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    let already_focused =
                        this.active_tab().is_some_and(|tab| tab.contains_focused(window, cx));
                    if this.file_navigation_revision == revision
                        && this.active_id() == id
                        && !window.has_active_dialog(cx)
                        && !already_focused
                    {
                        this.focus_active(window, cx);
                    }
                });
            });
        });
    }

    pub fn focus_active(&self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            return;
        }
        self.focus_handle.focus(window, cx);
        if !self.is_sessions_visible() {
            return;
        }
        let id = self.active_id();
        let revision = self.file_navigation_revision;
        let owner = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |this, cx| {
                if !this.is_sessions_visible()
                    || this.file_navigation_revision != revision
                    || this.active_id() != id
                    || !this.focus_handle.is_focused(window)
                {
                    return;
                }
                if let Some(tab) = this.active_tab() {
                    tab.focus(window, cx);
                }
            });
        });
    }

    pub fn try_activate(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if index >= self.tabs.items.len() || !self.begin_navigation(window, cx) {
            return false;
        }
        self.activate(index, window, cx);
        true
    }

    pub fn activate_next(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.activate_adjacent(true, window, cx)
    }

    pub fn activate_previous(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.activate_adjacent(false, window, cx)
    }

    fn activate_adjacent(
        &mut self,
        next: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(index) = adjacent_tab(self.tabs.active, self.tabs.items.len(), next) else {
            return false;
        };
        self.try_activate(index, window, cx)
    }
    pub fn show_diff(
        &mut self,
        session: &WeakEntity<SessionView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = session.upgrade() else {
            return;
        };
        let diff = session.read(cx).git_panel().diff().clone();
        if !diff.read(cx).is_open() {
            return;
        }
        let existing =
            self.tabs.items.iter().position(|tab| {
                tab.diff().is_some_and(|view| view.entity_id() == diff.entity_id())
            });
        let index = if let Some(index) = existing {
            index
        } else {
            let id = self.tabs.alloc_id();
            let subscription = Self::observe_diff(id, &diff, window, cx);
            let insert = self
                .tabs
                .items
                .iter()
                .position(|tab| {
                    tab.session().is_some_and(|view| view.entity_id() == session.entity_id())
                })
                .map(|index| index + 1)
                .unwrap_or(self.tabs.items.len());
            let tab = Tab::new(id, diff, subscription);
            self.tabs.items.insert(insert, WorkspaceTab::new_diff(DiffTab::new(tab, session)));
            insert
        };
        self.invalidate_file_navigation(cx);
        self.try_activate(index, window, cx);
        cx.notify();
    }

    pub fn observe_diff(
        id: usize,
        view: &Entity<GitDiffView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(view, window, move |_, view, _: &DiffTargetChanged, window, cx| {
            if view.read(cx).is_open() {
                cx.notify();
                return;
            }
            cx.defer_in(window, move |this, _, cx| {
                if this.tabs.items.iter().any(|tab| tab.id() == id && !tab.is_pending(cx)) {
                    this.close_tab(id, cx);
                }
            });
        })
    }
}

fn adjacent_tab(active: usize, count: usize, next: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(if next { (active + 1) % count } else { (active + count - 1) % count })
}
