use std::path::{Path, PathBuf};

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::files::{CloseFile, FileActionEvent, FileView, OpenFile};
use crate::workspace::workspace_view::{
    FileTab, FileTabAction, SidebarPanel, Tab, TabCommand, WorkspaceTab, WorkspaceView,
};

impl WorkspaceView {
    pub fn open_file(&mut self, target: OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if !self.begin_navigation(window, cx) {
            return;
        }
        let path = self.normalize_file_path(target.path(), cx);
        let existing = self.tabs.items.iter().position(|tab| {
            let WorkspaceTab::File(file) = tab else {
                return false;
            };
            file.view()
                .read(cx)
                .target()
                .is_some_and(|current| self.normalize_file_path(current.path(), cx) == path)
        });
        self.sidebar.panel = SidebarPanel::files();
        let index = if let Some(index) = existing {
            index
        } else {
            let id = self.tabs.alloc_id();
            let view = cx.new(|cx| FileView::new(window, cx));
            let subscription = cx.observe_in(&view, window, move |_, _, window, cx| {
                cx.notify();
                window.refresh();
                cx.defer_in(window, move |this, window, cx| {
                    this.on_file_updated(id, window, cx);
                });
            });
            let close_subscription =
                cx.subscribe_in(&view, window, move |_, _, _: &CloseFile, window, cx| {
                    cx.defer_in(window, move |this, window, cx| {
                        this.execute_tab_command(TabCommand::close(id), window, cx);
                    });
                });
            view.update(cx, |view, cx| view.open(target.clone(), window, cx));
            let tab = Tab::new(id, view, subscription);
            self.tabs.items.push(WorkspaceTab::new_file(FileTab::new(tab, close_subscription)));
            self.resolve_file_identity(id, window, cx);
            self.tabs.items.len() - 1
        };
        let Some(id) = self.tabs.tab(index).map(WorkspaceTab::id) else {
            return;
        };
        self.try_activate(index, window, cx);
        if self.active_id() != Some(id) {
            return;
        }
        if let Some(view) = self.active_file_view().cloned() {
            if let Some(line) = target.line().filter(|_| existing.is_some())
                && let Some(current) = view.read(cx).target().cloned()
            {
                let target =
                    OpenFile::new(current.directory().to_owned(), current.path().to_owned())
                        .with_line(line);
                self.reveal_file_tab(id, target, window, cx);
            }
            if let Some(target) = view.read(cx).target().cloned() {
                self.file_tree.update(cx, |tree, cx| tree.reveal(target.path(), cx));
            }
        }
        cx.notify();
    }

    pub fn reveal_file_tab(
        &self,
        id: usize,
        target: OpenFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let revision = self.file_navigation_revision;
        let owner = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.is_sessions_visible()
                    && this.file_navigation_revision == revision
                    && this.active_id() == Some(id)
                    && let Some(view) = this
                        .tabs
                        .items
                        .iter()
                        .find(|tab| tab.id() == id)
                        .and_then(WorkspaceTab::file)
                    && (this.focus_handle.is_focused(window)
                        || view.read(cx).focus_handle(cx).contains_focused(window, cx))
                {
                    view.update(cx, |view, cx| view.reveal(target, window, cx));
                }
            });
        });
    }

    pub fn normalize_file_path(&self, path: &Path, cx: &App) -> PathBuf {
        self.file_tree.read(cx).normalize_path(path)
    }

    pub fn resolve_file_identity(
        &mut self,
        id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.items.iter_mut().find(|tab| tab.id() == id) else {
            return;
        };
        let WorkspaceTab::File(file) = tab else {
            return;
        };
        file.reset_identity();
        let Some(target) = file.view().read(cx).target() else {
            return;
        };
        let path = target.path().to_owned();
        let task = cx.spawn_in(window, async move |this, cx| {
            let request = path.clone();
            let canonical =
                cx.background_spawn(async move { std::fs::canonicalize(request).ok() }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(tab) = this.tabs.items.iter_mut().find(|tab| tab.id() == id) else {
                    return;
                };
                let WorkspaceTab::File(file) = tab else {
                    return;
                };
                if file.view().read(cx).target().is_none_or(|target| target.path() != path) {
                    return;
                }
                file.settle_identity(canonical.clone());
                let Some(canonical) = canonical else {
                    return;
                };
                let mut duplicates: Vec<_> = this
                    .tabs
                    .items
                    .iter()
                    .filter_map(|tab| match tab {
                        WorkspaceTab::File(file) if file.canonical_path() == Some(&canonical) => {
                            Some(tab.id())
                        },
                        _ => None,
                    })
                    .collect();
                if !!this.files.pending.is_empty() {
                    cx.notify();
                    return;
                }
                duplicates.sort_unstable();
                let protected: Vec<_> = this
                    .tabs
                    .items
                    .iter()
                    .filter(|tab| duplicates.contains(&tab.id()))
                    .filter(|tab| {
                        tab.file().is_some_and(|view| {
                            view.read(cx).is_dirty() || view.read(cx).is_saving()
                        })
                    })
                    .map(WorkspaceTab::id)
                    .collect();
                let Some(keep) = protected.first().or(duplicates.first()).copied() else {
                    return;
                };
                let remove: Vec<_> = duplicates
                    .into_iter()
                    .filter(|id| *id != keep && !protected.contains(id))
                    .collect();
                if remove.is_empty() {
                    return;
                }
                let active_id = this.active_id();
                let replaced = active_id.is_some_and(|id| remove.contains(&id));
                let focused = this.active_file_view().is_some_and(|view| {
                    view.read(cx).focus_handle(cx).contains_focused(window, cx)
                }) || this.focus_handle.is_focused(window);
                let line = this
                    .selected_file_view()
                    .and_then(|view| view.read(cx).target())
                    .and_then(OpenFile::line);
                this.tabs.items.retain(|tab| !remove.contains(&tab.id()));
                let active_id = if replaced { Some(keep) } else { active_id };
                this.tabs.active = active_id.and_then(|id| this.tab_index(id)).unwrap_or(0);
                if replaced {
                    this.invalidate_file_navigation(cx);
                    this.sync_sidebar_active_session(cx);
                    this.tabs.scroll.scroll_to_item(this.tabs.active);
                    if focused {
                        this.focus_active(window, cx);
                        if let Some(line) = line
                            && let Some(view) = this.active_file_view()
                            && let Some(current) = view.read(cx).target().cloned()
                        {
                            let target = OpenFile::new(
                                current.directory().to_owned(),
                                current.path().to_owned(),
                            )
                            .with_line(line);
                            this.reveal_file_tab(keep, target, window, cx);
                        }
                    }
                }
                cx.notify();
            });
        });
        file.start_identity(task);
    }

    pub fn on_file_action(
        &mut self,
        event: FileActionEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let actions: Vec<_> = {
            let tree = self.file_tree.read(cx);
            let event = match event {
                FileActionEvent::Renamed { from, to } => {
                    FileActionEvent::renamed(tree.normalize_path(&from), tree.normalize_path(&to))
                },
                FileActionEvent::Deleted(path) => {
                    FileActionEvent::deleted(tree.normalize_path(&path))
                },
                FileActionEvent::Refresh(path) => {
                    FileActionEvent::refresh(tree.normalize_path(&path))
                },
                FileActionEvent::Created(path) => {
                    FileActionEvent::created(tree.normalize_path(&path))
                },
            };
            self.tabs
                .items
                .iter()
                .filter_map(|tab| {
                    let WorkspaceTab::File(file) = tab else {
                        return None;
                    };
                    let target = file.pending_target().or_else(|| file.view().read(cx).target())?;
                    let primary = tree.normalize_path(target.path());
                    let canonical = file.canonical_path().map(|path| tree.normalize_path(path));
                    let action = match &event {
                        FileActionEvent::Renamed { from, to } => {
                            if let Ok(relative) = primary.strip_prefix(from) {
                                let directory = tree.normalize_path(target.directory());
                                let directory = directory
                                    .strip_prefix(from)
                                    .map(|relative| {
                                        if relative.as_os_str().is_empty() {
                                            to.clone()
                                        } else {
                                            to.join(relative)
                                        }
                                    })
                                    .unwrap_or_else(|_| target.directory().to_owned());
                                let path = if relative.as_os_str().is_empty() {
                                    to.clone()
                                } else {
                                    to.join(relative)
                                };
                                let mut updated = OpenFile::new(directory, path);
                                if let Some(line) = target.line() {
                                    updated = updated.with_line(line);
                                }
                                FileTabAction::retarget(updated)
                            } else if canonical.as_ref().is_some_and(|path| path.starts_with(from))
                            {
                                FileTabAction::retarget(target.clone())
                            } else {
                                return None;
                            }
                        },
                        FileActionEvent::Deleted(path) => {
                            if primary.starts_with(path)
                                || canonical
                                    .as_ref()
                                    .is_some_and(|canonical| canonical.starts_with(path))
                            {
                                FileTabAction::close()
                            } else {
                                return None;
                            }
                        },
                        FileActionEvent::Refresh(path) => {
                            if primary.starts_with(path)
                                || canonical
                                    .as_ref()
                                    .is_some_and(|canonical| canonical.starts_with(path))
                            {
                                FileTabAction::reload()
                            } else {
                                return None;
                            }
                        },
                        FileActionEvent::Created(_) => return None,
                    };
                    Some((tab.id(), action))
                })
                .collect()
        };
        for (id, action) in actions {
            match action {
                FileTabAction::Retarget(target) => self.retarget_file_tab(id, target, window, cx),
                FileTabAction::Reload => self.refresh_file_tab(id, window, cx),
                FileTabAction::Close => self.close_tab(id, cx),
            }
        }
        self.sync_file_directory(cx);
        cx.notify();
    }

    pub fn file_view(&self, id: usize) -> Option<&Entity<FileView>> {
        self.tabs.find(id).and_then(WorkspaceTab::file)
    }

    pub fn refresh_file_tab(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.file_view(id).cloned() else {
            return;
        };
        if view.read(cx).is_dirty() || view.read(cx).is_saving() {
            return;
        }
        let focus = window.focused(cx);
        view.update(cx, |view, cx| view.reload(window, cx));
        if let Some(focus) = focus {
            focus.focus(window, cx);
        }
        self.resolve_file_identity(id, window, cx);
    }

    pub fn retarget_file_tab(
        &mut self,
        id: usize,
        target: OpenFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.items.iter_mut().find(|tab| tab.id() == id) else {
            return;
        };
        let WorkspaceTab::File(file) = tab else {
            return;
        };
        file.reset_identity();
        if file.view().read(cx).is_saving() {
            file.set_pending_target(target);
            return;
        }
        file.take_pending_target();
        let focus = window.focused(cx);
        file.view().update(cx, |view, cx| view.retarget(target, window, cx));
        if let Some(focus) = focus {
            focus.focus(window, cx);
        }
        self.resolve_file_identity(id, window, cx);
    }

    pub fn save_file_tab(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.begin_navigation(window, cx) {
            return;
        }
        if let Some(view) = self.file_view(id) {
            view.update(cx, |view, cx| view.save(window, cx));
        }
    }

    pub fn on_file_updated(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.tabs.items.iter_mut().find_map(|tab| {
            if tab.id() != id {
                return None;
            }
            let WorkspaceTab::File(file) = tab else {
                return None;
            };
            (!file.view().read(cx).is_saving()).then(|| file.take_pending_target()).flatten()
        });
        if let Some(target) = target {
            self.retarget_file_tab(id, target, window, cx);
            self.sync_file_directory(cx);
        }
        if self
            .files
            .confirmation
            .as_ref()
            .is_some_and(|confirmation| confirmation.id == id && confirmation.saving_requested)
            && let Some(view) = self.file_view(id).cloned()
            && !view.read(cx).is_saving()
        {
            if view.read(cx).save_error().is_none() {
                if view.read(cx).is_dirty() {
                    view.update(cx, |view, cx| view.save(window, cx));
                } else {
                    self.finish_file_confirmation(id, window, cx);
                    return;
                }
            } else if let Some(confirmation) = self.files.confirmation.as_mut() {
                confirmation.saving_requested = false;
                window.refresh();
                cx.notify();
            }
        }
        if self.files.confirmation.is_none() && self.files.pending.is_empty() {
            self.advance_file_operations(window, cx);
        }
    }

    pub fn file_navigation_revision(&self) -> u64 {
        self.file_navigation_revision
    }

    pub fn is_active_session(&self, owner: &WeakEntity<SessionView>) -> bool {
        self.active_view().is_some_and(|view| owner.upgrade().as_ref() == Some(view))
    }

    pub fn invalidate_file_navigation(&mut self, _: &mut Context<Self>) {
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
    }
}
