use gpui_kit::ScrollHandle;

use crate::workspace::SessionView;
use crate::workspace::workspace_view::{Tab, WorkspaceTab};

pub struct TabSet {
    pub items: Vec<WorkspaceTab>,
    pub archived: Vec<Tab<SessionView>>,
    pub active: usize,
    pub next_id: usize,
    pub scroll: ScrollHandle,
}

impl TabSet {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            archived: Vec::new(),
            active: 0,
            next_id: 0,
            scroll: ScrollHandle::new(),
        }
    }

    pub fn alloc_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn tab(&self, index: usize) -> Option<&WorkspaceTab> {
        self.items.get(index)
    }

    pub fn find(&self, id: usize) -> Option<&WorkspaceTab> {
        self.items.iter().find(|tab| tab.id() == id)
    }

    pub fn active_tab(&self) -> Option<&WorkspaceTab> {
        self.items.get(self.active)
    }

    pub fn active_id(&self) -> Option<usize> {
        self.active_tab().map(WorkspaceTab::id)
    }

    pub fn index_of(&self, id: usize) -> Option<usize> {
        self.items.iter().position(|tab| tab.id() == id)
    }
}
