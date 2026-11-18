use crate::workspace::files::file_tree::*;

impl FileTree {
    pub fn start_search(
        &mut self,
        value: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.search.active {
            self.remember_selection(cx);
            self.cancel_reveal();
            self.search.saved_selection = self.selected.clone();
            self.search.active = true;
            self.search.query.clear();
            self.search.input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        if let Some(value) = value {
            self.search.input.update(cx, |input, cx| input.set_value(value, window, cx));
            self.query_changed(cx);
        }
        self.search.input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    pub fn find_entry(&mut self, _: &FindEntry, window: &mut Window, cx: &mut Context<Self>) {
        if self.keyboard_available(window, cx) {
            self.start_search(None, window, cx);
        } else {
            cx.propagate();
        }
    }

    pub fn leave_search(&mut self, cx: &mut Context<Self>) {
        if !self.search.active {
            return;
        }
        self.search.active = false;
        self.search.query.clear();
        self.search.revision += 1;
        self.search.filter_task = None;
        self.selected = self.search.saved_selection.take();
        self.rebuild(cx);
    }

    pub fn escape_search(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.active && self.keyboard_available(window, cx) {
            self.leave_search(cx);
            self.focus(window, cx);
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }

    pub fn typed_search(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke;
        if !self.tree_focused(window, cx)
            || !self.keyboard_available(window, cx)
            || key.modifiers.platform
            || key.modifiers.control
            || key.modifiers.alt
            || key.modifiers.function
        {
            return;
        }
        let Some(text) = key
            .key_char
            .as_deref()
            .filter(|text| !text.is_empty() && text.chars().all(|ch| !ch.is_control()))
        else {
            return;
        };
        let mut value = if self.search.active {
            self.search.input.read(cx).value().to_string()
        } else {
            String::new()
        };
        value.push_str(text);
        self.start_search(Some(&value), window, cx);
        cx.stop_propagation();
    }

    pub fn query_changed(&mut self, cx: &mut Context<Self>) {
        if !self.search.active {
            return;
        }
        let query = self.search.input.read(cx).value().trim().to_lowercase();
        if query == self.search.query {
            return;
        }
        self.search.query = query;
        self.search.revision += 1;
        self.search.filter_task = None;
        self.search.results.clear();
        self.search.truncated = false;
        self.search.error = None;
        if self.search.query.is_empty() {
            self.selected = self.search.saved_selection.clone();
        }
        self.schedule_search(cx);
        self.rebuild(cx);
    }

    pub fn schedule_search(&mut self, cx: &mut Context<Self>) {
        if !self.search.has_query() {
            return;
        }
        let revision = self.search.revision;
        let generation = self.generation;
        self.search.filter_task = Some(cx.spawn(async move |view, cx| {
            cx.background_executor().timer(SEARCH_DEBOUNCE).await;
            let _ = view.update(cx, |this, cx| {
                if this.search.revision == revision
                    && this.generation == generation
                    && this.search.has_query()
                {
                    this.search.filter_task = None;
                    this.search_cached_index(cx);
                }
            });
        }));
    }

    pub fn search_cached_index(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.search.index.clone() else {
            if self.search.index_task.is_some() {
                return;
            }
            let root = self.root.clone();
            let generation = self.generation;
            self.search.error = None;
            self.search.index_task = Some(cx.spawn(async move |view, cx| {
                let result = cx.background_spawn(async move { read_search_index(&root) }).await;
                let _ = view.update(cx, |this, cx| {
                    if this.generation != generation {
                        return;
                    }
                    this.search.index_task = None;
                    match result {
                        Ok(index) => {
                            this.search.index = Some(Arc::new(index));
                            if this.search.has_query() {
                                this.search_cached_index(cx);
                            }
                        },
                        Err(error) => this.search.error = Some(error.to_string()),
                    }
                    this.rebuild(cx);
                });
            }));
            self.rebuild(cx);
            return;
        };
        let query = self.search.query.clone();
        let root = self.root.clone();
        let revision = self.search.revision;
        let generation = self.generation;
        self.search.filter_task = Some(cx.spawn(async move |view, cx| {
            let mut results = cx
                .background_spawn(async move {
                    index
                        .iter()
                        .filter(|entry| {
                            entry
                                .path
                                .strip_prefix(&root)
                                .unwrap_or(&entry.path)
                                .to_string_lossy()
                                .to_lowercase()
                                .contains(&query)
                        })
                        .take(SEARCH_RESULT_LIMIT + 1)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation
                    || this.search.revision != revision
                    || !this.search.has_query()
                {
                    return;
                }
                this.remember_selection(cx);
                this.search.filter_task = None;
                this.search.truncated = results.len() > SEARCH_RESULT_LIMIT;
                results.truncate(SEARCH_RESULT_LIMIT);
                if !results.iter().any(|entry| Some(&entry.path) == this.selected.as_ref()) {
                    this.selected = results.first().map(|entry| entry.path.clone());
                }
                this.search.results = results;
                this.rebuild(cx);
            });
        }));
    }

    pub fn activate_search_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search.has_query() {
            return;
        }
        let Some(entry) = self.selected_entry(cx) else {
            return;
        };
        self.leave_search(cx);
        self.focus(window, cx);
        if entry.directory {
            self.expanded.insert(entry.path.clone());
        } else {
            cx.emit(OpenFile::new(self.directory.clone(), entry.path.clone()));
        }
        self.reveal(&entry.path, cx);
    }

    pub fn navigate_search(&mut self, previous: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.has_query()
            && self.search_input_focused(window, cx)
            && self.keyboard_available(window, cx)
        {
            self.select_enabled(previous, cx);
            cx.stop_propagation();
        }
    }
}

fn read_search_index(root: &Path) -> Result<Vec<DiskEntry>, FileTreeError> {
    let mut contents = DirectoryContents::new();
    contents.read_entries(root, true, None)?;
    contents.entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(contents.entries)
}
