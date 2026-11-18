use crate::workspace::files::file_tree::*;

impl FileTree {
    pub fn reveal(&mut self, path: &Path, cx: &mut Context<Self>) {
        if self.search.active {
            self.leave_search(cx);
        }
        self.remember_selection(cx);
        self.cancel_reveal();
        self.reveal_error = None;
        let path = if path.is_absolute() { path.to_owned() } else { self.root.join(path) };
        match absolute_path(&path) {
            Ok(path) => {
                self.pending_reveal = Some(RevealRequest::new(path));
                self.resolve_reveal(cx);
            },
            Err(error) => self.reveal_error = Some(error),
        }
        self.rebuild(cx);
    }

    pub fn cancel_reveal(&mut self) {
        self.reveal_id += 1;
        self.reveal_task = None;
        self.pending_reveal = None;
    }

    pub fn resolve_reveal(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.pending_reveal.as_ref() else {
            return;
        };
        let path = request.path.clone();
        let root = self.root.clone();
        let generation = self.generation;
        let reveal_id = self.reveal_id;
        self.reveal_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    let resolved_root = fs::canonicalize(&root)
                        .map_err(|error| FileTreeError::read(&root, error))?;
                    let target = fs::symlink_metadata(&path)
                        .and_then(|metadata| {
                            if path == root {
                                Ok(resolved_root.clone())
                            } else if metadata.file_type().is_symlink() {
                                let parent = path
                                    .parent()
                                    .ok_or_else(|| io::Error::other("路径没有父目录"))?;
                                let name = path
                                    .file_name()
                                    .ok_or_else(|| io::Error::other("路径没有名称"))?;
                                fs::canonicalize(parent).map(|parent| parent.join(name))
                            } else {
                                fs::canonicalize(&path)
                            }
                        })
                        .map_err(|error| {
                            if error.kind() == io::ErrorKind::NotFound {
                                FileTreeError::missing(&path)
                            } else {
                                FileTreeError::read(&path, error)
                            }
                        })?;
                    let relative = target
                        .strip_prefix(&resolved_root)
                        .map_err(|_| FileTreeError::outside_root(&path))?;
                    Ok::<_, FileTreeError>(root.join(relative))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation || this.reveal_id != reveal_id {
                    return;
                }
                this.reveal_task = None;
                this.remember_selection(cx);
                match result {
                    Ok(target) if target == this.root => {
                        this.selected = None;
                        this.pending_reveal = None;
                    },
                    Ok(target) => {
                        if let Some(request) = this.pending_reveal.as_mut() {
                            request.target = Some(target);
                        }
                    },
                    Err(error) => {
                        this.pending_reveal = None;
                        this.reveal_error = Some(error);
                    },
                }
                this.advance_reveal(cx);
                this.rebuild(cx);
            });
        }));
    }

    pub fn remember_selection(&mut self, cx: &App) {
        if let Some(entry) =
            self.state.read(cx).selected_item().and_then(|item| self.paths.get(&item.id))
        {
            self.selected = Some(entry.path.clone());
        }
    }

    pub fn advance_reveal(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.pending_reveal.as_ref().and_then(|request| request.target.clone())
        else {
            return;
        };
        let Ok(relative) = target.strip_prefix(&self.root) else {
            self.pending_reveal = None;
            self.reveal_error = Some(FileTreeError::outside_root(&target));
            return;
        };
        let mut parent = self.root.clone();
        let mut components = relative.components().peekable();
        while let Some(component) = components.next() {
            let contents = match self.directories.get(&parent) {
                None | Some(DirectoryContents { status: DirectoryStatus::Unloaded, .. }) => {
                    self.request_directory(parent, cx);
                    return;
                },
                Some(contents) if matches!(contents.status, DirectoryStatus::Loading) => return,
                Some(contents) => contents,
            };
            let path = parent.join(component);
            let Some(entry) = contents.entries.iter().find(|entry| entry.path == path) else {
                self.reveal_error =
                    Some(if matches!(contents.status, DirectoryStatus::Failed(_)) {
                        FileTreeError::reveal_blocked(&parent)
                    } else {
                        FileTreeError::missing(&target)
                    });
                self.pending_reveal = None;
                return;
            };
            if components.peek().is_none() {
                self.selected = Some(path.clone());
                let expanded = entry.directory && self.expanded.contains(&path);
                self.pending_reveal = None;
                if expanded {
                    self.request_expanded(cx);
                }
                self.rebuild(cx);
                let id = path_id(&target);
                self.state.update(cx, |state, cx| {
                    if let Some(ix) = state.index_of(&id) {
                        state.set_selected_index(Some(ix), cx);
                        state.scroll_to_item(ix, ScrollStrategy::Center);
                    }
                });
                return;
            }
            if !entry.directory {
                self.reveal_error = Some(FileTreeError::missing(&target));
                self.pending_reveal = None;
                return;
            }
            self.expanded.insert(path.clone());
            parent = path;
        }
    }

    pub fn normalize_path(&self, path: &Path) -> PathBuf {
        let path = if path.is_absolute() { path.to_owned() } else { self.root.join(path) };
        map_root_path(&path, &self.root, self.resolved_root.as_deref())
    }

    pub fn track_current_file(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        let path = path.map(|path| self.normalize_path(&path));
        if self.current_file != path {
            self.current_file = path;
            cx.notify();
        }
    }

    pub fn selected_entry(&self, cx: &App) -> Option<DiskEntry> {
        self.state.read(cx).selected_item().and_then(|item| self.paths.get(&item.id)).cloned()
    }
}
