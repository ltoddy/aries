use crate::workspace::files::file_tree::render::{build_items, status_item};
use crate::workspace::files::file_tree::*;

impl FileTree {
    pub fn new(directory: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| TreeState::new(cx));
        let tree_subscription = cx.subscribe(&state, |this, _, event, cx| {
            let event = event.clone();
            let generation = this.generation;
            let view = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = view.update(cx, |this, cx| {
                    if this.generation == generation {
                        this.on_tree_event(&event, cx);
                    }
                });
            });
        });
        let actions = cx.new(FileActions::new);
        let action_subscription = cx.subscribe(&actions, |this, _, event, cx| {
            let event = event.clone();
            let root = this.action_root.clone();
            let resolved = this.action_resolved_root.clone();
            let view = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = view.update(cx, |this, cx| {
                    if this.root == root {
                        this.on_file_action(event, resolved.as_deref(), cx);
                    }
                });
            });
        });
        let action_observation = cx.observe(&actions, |_, _, cx| cx.notify());
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("文件名或相对路径"));
        let input_subscription = cx.subscribe_in(&input, window, |_, _, event, window, cx| {
            if matches!(event, InputEvent::Focus | InputEvent::Blur) {
                cx.notify();
                return;
            }
            let change = matches!(event, InputEvent::Change);
            let enter = matches!(event, InputEvent::PressEnter { secondary: false, shift: false });
            if !change && !enter {
                return;
            }
            let view = cx.weak_entity();
            window.defer(cx, move |window, cx| {
                let _ = view.update(cx, |this, cx| {
                    if change {
                        this.query_changed(cx);
                    } else if this.search.active
                        && this.search_input_focused(window, cx)
                        && this.keyboard_available(window, cx)
                    {
                        this.activate_search_result(window, cx);
                    }
                });
            });
        });
        let view = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = view.update(cx, |this, cx| {
                if this.generation == 0 {
                    this.start_watch(cx);
                    this.load(cx);
                }
            });
        });
        let root = absolute_path(&directory).unwrap_or_else(|_| directory.clone());
        Self {
            directory,
            action_root: root.clone(),
            root,
            state,
            actions,
            action_resolved_root: None,
            resolved_root: None,
            git_status: TreeGitStatus::new(),
            git_error: None,
            git_task: None,
            watch_task: None,
            watch_error: None,
            search: FileSearch::new(input),
            current_file: None,
            _action_subscription: action_subscription,
            _action_observation: action_observation,
            _input_subscription: input_subscription,
            directories: HashMap::new(),
            paths: HashMap::new(),
            expanded: HashSet::new(),
            selected: None,
            pending_reveal: None,
            reveal_error: None,
            reveal_id: 0,
            reveal_task: None,
            generation: 0,
            tasks: HashMap::new(),
            focus: cx.focus_handle().tab_stop(true),
            _tree_subscription: tree_subscription,
            focus_subscription: None,
            focus_out_subscription: None,
        }
    }

    pub fn set_directory(&mut self, directory: PathBuf, cx: &mut Context<Self>) {
        if self.directory == directory {
            return;
        }
        self.root = absolute_path(&directory).unwrap_or_else(|_| directory.clone());
        self.directory = directory;
        self.resolved_root = None;
        self.git_status = TreeGitStatus::new();
        self.search.active = false;
        self.search.query.clear();
        self.search.saved_selection = None;
        self.current_file = None;
        self.directories.clear();
        self.paths.clear();
        self.expanded.clear();
        self.selected = None;
        self.cancel_reveal();
        self.reveal_error = None;
        self.state.update(cx, |state, cx| state.set_items(Vec::new(), cx));
        self.start_watch(cx);
        self.load(cx);
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        if self.watch_error.is_some() {
            self.start_watch(cx);
        }
        self.remember_selection(cx);
        self.generation += 1;
        self.search.invalidate();
        self.load_git_status(cx);
        if self.search.has_query() {
            self.schedule_search(cx);
        }
        self.tasks.clear();
        self.reveal_task = None;
        if let Some(request) = self.pending_reveal.as_mut() {
            request.target = None;
        }
        self.reveal_error = None;
        for contents in self.directories.values_mut() {
            contents.status = DirectoryStatus::unloaded();
        }
        self.request_directory(self.root.clone(), cx);
        self.resolve_reveal(cx);
        self.rebuild(cx);
    }

    pub fn request_directory(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let contents = self.directories.entry(path.clone()).or_insert_with(DirectoryContents::new);
        if !matches!(contents.status, DirectoryStatus::Unloaded) {
            return;
        }
        contents.status = DirectoryStatus::loading();
        let generation = self.generation;
        let root = path == self.root;
        let request_path = path.clone();
        let task = cx.spawn(async move |view, cx| {
            let read_path = request_path.clone();
            let contents =
                cx.background_spawn(async move { DirectoryContents::read(&read_path, root) }).await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.remember_selection(cx);
                this.tasks.remove(&request_path);
                let mut contents = contents;
                if matches!(
                    contents.status,
                    DirectoryStatus::Failed(
                        FileTreeError::Read { .. } | FileTreeError::Walk { .. }
                    )
                ) && contents.entries.is_empty()
                    && let Some(previous) = this.directories.get_mut(&request_path)
                {
                    contents.entries = std::mem::take(&mut previous.entries);
                }
                this.directories.insert(request_path, contents);
                this.request_expanded(cx);
                this.advance_reveal(cx);
                this.rebuild(cx);
            });
        });
        self.tasks.insert(path, task);
    }

    pub fn request_expanded(&mut self, cx: &mut Context<Self>) {
        let mut pending = vec![self.root.clone()];
        let mut requests = Vec::new();
        while let Some(path) = pending.pop() {
            match self.directories.get(&path) {
                None | Some(DirectoryContents { status: DirectoryStatus::Unloaded, .. }) => {
                    requests.push(path)
                },
                Some(contents)
                    if matches!(
                        contents.status,
                        DirectoryStatus::Loaded
                            | DirectoryStatus::Failed(
                                FileTreeError::Read { .. } | FileTreeError::Walk { .. }
                            )
                    ) =>
                {
                    pending.extend(
                        contents
                            .entries
                            .iter()
                            .filter(|entry| entry.directory && self.expanded.contains(&entry.path))
                            .map(|entry| entry.path.clone()),
                    );
                },
                Some(_) => {},
            }
        }
        for path in requests {
            self.request_directory(path, cx);
        }
    }

    pub fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.paths.clear();
        let items = if self.search.has_query() {
            let mut items: Vec<_> = self
                .search
                .results
                .iter()
                .map(|entry| {
                    let id = path_id(&entry.path);
                    self.paths.insert(id.clone(), entry.clone());
                    TreeItem::new(
                        id,
                        entry
                            .path
                            .strip_prefix(&self.root)
                            .unwrap_or(&entry.path)
                            .to_string_lossy()
                            .into_owned(),
                    )
                })
                .collect();
            if items.is_empty() {
                items.push(status_item(
                    &self.root,
                    if self.search.index_task.is_some() || self.search.filter_task.is_some() {
                        "正在搜索项目…".into()
                    } else if let Some(error) = &self.search.error {
                        error.clone()
                    } else {
                        "没有匹配项，试试其他文件名或相对路径".into()
                    },
                ));
            }
            items
        } else {
            build_items(&self.root, &self.directories, &self.expanded, &mut self.paths)
        };
        let selected_id = self.selected.as_deref().map(path_id);
        self.state.update(cx, |state, cx| {
            state.set_items(items, cx);
            let ix = selected_id.as_ref().and_then(|id| state.index_of(id));
            state.set_selected_index(ix, cx);
        });
        cx.notify();
    }

    pub fn load_git_status(&mut self, cx: &mut Context<Self>) {
        let root = self.root.clone();
        let generation = self.generation;
        self.git_error = None;
        self.git_task = Some(cx.spawn(async move |view, cx| {
            let (resolved, status) = cx
                .background_spawn(async move {
                    (fs::canonicalize(&root).ok(), TreeGitStatus::read(&root))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.git_task = None;
                this.resolved_root = resolved;
                this.current_file = this
                    .current_file
                    .take()
                    .map(|path| map_root_path(&path, &this.root, this.resolved_root.as_deref()));
                match status {
                    Ok(status) => this.git_status = status,
                    Err(error) => {
                        this.git_status = TreeGitStatus::new();
                        this.git_error = Some(format!("Git 状态读取失败：{error}。刷新可重试。"));
                    },
                }
                cx.notify();
            });
        }));
    }
}
