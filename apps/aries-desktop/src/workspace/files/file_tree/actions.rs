use crate::workspace::files::file_tree::*;

impl FileTree {
    pub fn on_tree_event(&mut self, event: &TreeEvent, cx: &mut Context<Self>) {
        if self.search.has_query() {
            return;
        }
        let (TreeEvent::Expanded(id) | TreeEvent::Collapsed(id)) = event;
        let Some(entry) = self.paths.get(id).filter(|entry| entry.directory) else {
            return;
        };
        let path = entry.path.clone();
        self.remember_selection(cx);
        self.cancel_reveal();
        self.set_expanded(path, matches!(event, TreeEvent::Expanded(_)), cx);
        self.rebuild(cx);
    }

    pub fn set_expanded(&mut self, path: PathBuf, expanded: bool, cx: &mut Context<Self>) {
        if expanded {
            self.expanded.insert(path.clone());
            if let Some(contents) = self.directories.get_mut(&path)
                && matches!(contents.status, DirectoryStatus::Failed(_))
            {
                contents.status = DirectoryStatus::unloaded();
            }
            self.request_expanded(cx);
        } else {
            self.expanded.remove(&path);
        }
    }

    pub fn confirm_entry(&mut self, _: &ConfirmEntry, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tree_focused(window, cx) || !self.keyboard_available(window, cx) {
            cx.propagate();
            return;
        }
        if self.search.has_query() {
            self.activate_search_result(window, cx);
            return;
        }
        let Some(item) = self.state.read(cx).selected_item() else {
            return;
        };
        let Some(entry) = self.paths.get(&item.id).cloned() else {
            return;
        };
        self.cancel_reveal();
        self.reveal_error = None;
        self.selected = Some(entry.path.clone());
        if entry.directory {
            let expanded = !self.expanded.contains(&entry.path);
            self.set_expanded(entry.path, expanded, cx);
            self.rebuild(cx);
        } else {
            cx.emit(OpenFile::new(self.directory.clone(), entry.path));
        }
    }

    pub fn clicked_entry(
        &mut self,
        id: &SharedString,
        generation: u64,
        reveal_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if generation != self.generation
            || reveal_id != self.reveal_id
            || self.state.read(cx).selected_item().is_none_or(|item| &item.id != id)
        {
            return;
        }
        if self.search.has_query() {
            self.activate_search_result(window, cx);
            return;
        }
        self.cancel_reveal();
        self.reveal_error = None;
        if let Some(entry) = self.paths.get(id) {
            self.selected = Some(entry.path.clone());
            if !entry.directory {
                cx.emit(OpenFile::new(self.directory.clone(), entry.path.clone()));
            }
        }
        cx.notify();
    }

    pub fn select_enabled(&mut self, previous: bool, cx: &mut Context<Self>) {
        self.cancel_reveal();
        self.reveal_error = None;
        self.state.update(cx, |state, cx| {
            let mut len = 0;
            while state.entry(len).is_some() {
                len += 1;
            }
            let mut ix = state.selected_index().filter(|ix| *ix < len);
            for _ in 0..len {
                let next = if previous {
                    ix.and_then(|ix| ix.checked_sub(1)).unwrap_or(len - 1)
                } else {
                    ix.filter(|ix| ix + 1 < len).map_or(0, |ix| ix + 1)
                };
                if state.entry(next).is_some_and(|entry| !entry.is_disabled()) {
                    state.set_selected_index(Some(next), cx);
                    state.scroll_to_item(
                        next,
                        if previous { ScrollStrategy::Top } else { ScrollStrategy::Bottom },
                    );
                    return;
                }
                ix = Some(next);
            }
            state.set_selected_index(None, cx);
        });
        self.remember_selection(cx);
        cx.notify();
    }

    pub fn on_file_action(
        &mut self,
        event: FileActionEvent,
        resolved_root: Option<&Path>,
        cx: &mut Context<Self>,
    ) {
        let map = |path: &Path| map_root_path(path, &self.root, resolved_root);
        let event = match event {
            FileActionEvent::Created(path) => FileActionEvent::created(map(&path)),
            FileActionEvent::Renamed { from, to } => FileActionEvent::renamed(map(&from), map(&to)),
            FileActionEvent::Deleted(path) => FileActionEvent::deleted(map(&path)),
            FileActionEvent::Refresh(path) => FileActionEvent::refresh(map(&path)),
        };
        self.remember_selection(cx);
        self.leave_search(cx);
        self.cancel_reveal();
        let target = match &event {
            FileActionEvent::Created(path) => Some(path.clone()),
            FileActionEvent::Refresh(_) => self.selected.clone(),
            FileActionEvent::Renamed { from, to } => {
                self.expanded =
                    self.expanded.drain().map(|path| replace_prefix(path, from, to)).collect();
                self.selected = self.selected.take().map(|path| replace_prefix(path, from, to));
                self.current_file =
                    self.current_file.take().map(|path| replace_prefix(path, from, to));
                self.selected
                    .clone()
                    .filter(|path| path.starts_with(to))
                    .or_else(|| Some(to.clone()))
            },
            FileActionEvent::Deleted(path) => {
                self.expanded.retain(|expanded| !expanded.starts_with(path));
                if self.current_file.as_ref().is_some_and(|current| current.starts_with(path)) {
                    self.current_file = None;
                }
                self.selected = self.selected.take().filter(|selected| !selected.starts_with(path));
                self.selected.clone().or_else(|| path.parent().map(Path::to_owned))
            },
        };
        self.state.update(cx, |state, cx| state.set_selected_index(None, cx));
        self.directories.clear();
        self.load(cx);
        if let Some(target) = target {
            self.reveal(&target, cx);
        }
        let owner = cx.weak_entity();
        let root = self.root.clone();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.root == root {
                    cx.emit(event);
                }
            });
        });
    }

    pub fn cancel_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.actions.update(cx, |actions, cx| actions.cancel(window, cx))
    }

    pub fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.remember_selection(cx);
        self.cancel_reveal();
        self.expanded.clear();
        if self.search.has_query() {
            self.search.saved_selection = self
                .search
                .saved_selection
                .as_ref()
                .and_then(|path| path.strip_prefix(&self.root).ok())
                .and_then(|relative| relative.components().next())
                .map(|first| self.root.join(first));
        }
        if !self.search.has_query()
            && let Some(selected) = &self.selected
            && let Ok(relative) = selected.strip_prefix(&self.root)
            && let Some(first) = relative.components().next()
        {
            self.selected = Some(self.root.join(first));
        }
        self.rebuild(cx);
    }

    pub fn execute_command(
        &mut self,
        command: TreeCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx) {
            return;
        }
        match command {
            TreeCommand::File(command) => {
                if self.actions.read(cx).is_busy() || self.resolved_root.is_none() {
                    return;
                }
                self.action_root = self.root.clone();
                self.action_resolved_root = self.resolved_root.clone();
                self.actions.update(cx, |actions, cx| actions.execute(command, window, cx));
            },
            TreeCommand::Open(entry) => {
                self.leave_search(cx);
                self.focus(window, cx);
                cx.emit(OpenFile::new(self.directory.clone(), entry.path.clone()));
                self.reveal(&entry.path, cx);
            },
            TreeCommand::Expand(path) => {
                self.leave_search(cx);
                self.focus(window, cx);
                self.expanded.retain(|expanded| !expanded.starts_with(&path));
                self.expanded.insert(path.clone());
                self.reveal(&path, cx);
                self.request_expanded(cx);
            },
            TreeCommand::Collapse => self.collapse_all(cx),
            TreeCommand::Refresh => self.load(cx),
            TreeCommand::Locate => {
                if let Some(path) = self.current_file.clone() {
                    self.leave_search(cx);
                    self.reveal(&path, cx);
                    self.focus(window, cx);
                }
            },
            TreeCommand::Search => self.start_search(None, window, cx),
            TreeCommand::Copy(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
        }
    }

    pub fn rename_entry(&mut self, _: &RenameEntry, window: &mut Window, cx: &mut Context<Self>) {
        if self.tree_focused(window, cx) && self.keyboard_available(window, cx) {
            if let Some(entry) = self.selected_entry(cx).filter(|entry| entry.path != self.root) {
                self.execute_command(
                    TreeCommand::file(FileCommand::rename(self.root.clone(), entry.path)),
                    window,
                    cx,
                );
            }
        } else {
            cx.propagate();
        }
    }

    pub fn delete_entry(&mut self, _: &DeleteEntry, window: &mut Window, cx: &mut Context<Self>) {
        if self.tree_focused(window, cx) && self.keyboard_available(window, cx) {
            if let Some(entry) = self.selected_entry(cx).filter(|entry| entry.path != self.root) {
                self.execute_command(
                    TreeCommand::file(FileCommand::delete(self.root.clone(), entry.path)),
                    window,
                    cx,
                );
            }
        } else {
            cx.propagate();
        }
    }

    pub fn copy_relative_path(
        &mut self,
        _: &CopyRelativePath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tree_focused(window, cx) && self.keyboard_available(window, cx) {
            if let Some(entry) = self.selected_entry(cx) {
                let path = entry
                    .path
                    .strip_prefix(&self.root)
                    .unwrap_or(&entry.path)
                    .to_string_lossy()
                    .into_owned();
                cx.write_to_clipboard(ClipboardItem::new_string(path));
            }
        } else {
            cx.propagate();
        }
    }
}
