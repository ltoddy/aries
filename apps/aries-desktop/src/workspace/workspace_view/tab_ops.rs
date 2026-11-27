use crate::workspace::workspace_view::*;

impl WorkspaceView {
    pub fn tab_index(&self, id: usize) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.id() == id)
    }

    pub fn active_tab(&self) -> Option<&WorkspaceTab> {
        self.tabs.get(self.active)
    }

    pub fn active_id(&self) -> Option<usize> {
        self.active_tab().map(WorkspaceTab::id)
    }

    pub fn has_saving_file(&self, cx: &App) -> bool {
        self.tabs.iter().any(|tab| tab.is_saving(cx))
    }

    pub fn can_navigate(&self, window: &mut Window, cx: &mut App) -> bool {
        self.file_confirmation.is_none()
            && self.pending_file_operations.is_empty()
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

    pub fn activate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.active = index;
        self.tab_scroll.scroll_to_item(index);
        self.sync_sidebar_active_session(cx);
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    pub fn close_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        if self
            .file_view(id)
            .is_some_and(|view| view.read(cx).is_dirty() || view.read(cx).is_saving())
            || self.pending_file_operations.iter().any(|operation| operation.tab_id() == Some(id))
        {
            return;
        }
        self.remove_tab(id, cx);
    }

    pub fn remove_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(index) = self.tab_index(id) else {
            return;
        };
        let tab = &self.tabs[index];
        if tab.is_pending(cx) || tab.is_saving(cx) {
            return;
        }
        let active_id = self.active_id();
        let session = tab.session().map(|view| view.entity_id());
        let tab = self.tabs.remove(index);
        let background = match tab {
            WorkspaceTab::Session(tab) if !tab.view().read(cx).is_session_idle() => Some(tab),
            _ => None,
        };
        match background {
            Some(tab) => self.background_tabs.push(tab),
            None => {
                if let Some(session) = session {
                    self.tabs.retain(|tab| {
                        tab.diff_session().is_none_or(|view| view.entity_id() != session)
                    });
                }
            },
        }
        self.active = active_id
            .and_then(|id| self.tab_index(id))
            .unwrap_or_else(|| index.min(self.tabs.len().saturating_sub(1)));
        let changed_active = active_id != self.active_id();
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

    pub fn tab_order(&self) -> Vec<usize> {
        self.tabs.iter().map(WorkspaceTab::id).collect()
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
        self.tabs.sort_by_key(|tab| order.iter().position(|id| *id == tab.id()));
        self.active = active.and_then(|id| self.tab_index(id)).unwrap_or(0);
        self.tab_scroll.scroll_to_item(to);
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
                        this.focus_active_tab(window, cx);
                    }
                });
            });
        });
    }

    pub fn focus_active_tab(&self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub fn select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() || !self.begin_navigation(window, cx) {
            return;
        }
        self.activate_tab(index, window, cx);
    }

    pub fn switch_tab(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = adjacent_tab(self.active, self.tabs.len(), next) else {
            return;
        };
        self.select_tab(index, window, cx);
    }
}

fn adjacent_tab(active: usize, count: usize, next: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(if next { (active + 1) % count } else { (active + count - 1) % count })
}
