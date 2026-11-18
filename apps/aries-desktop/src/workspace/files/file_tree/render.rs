use crate::workspace::files::file_tree::*;

impl FileTree {
    pub fn tree_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx) && !GlobalState::is_in_deferred_context(cx)
    }

    pub fn search_input_focused(&self, window: &Window, cx: &App) -> bool {
        self.search.input.focus_handle(cx).is_focused(window)
    }

    pub fn keyboard_available(&self, window: &mut Window, cx: &mut App) -> bool {
        (self.focus.contains_focused(window, cx) || self.search_input_focused(window, cx))
            && !window.has_active_dialog(cx)
            && !GlobalState::is_in_deferred_context(cx)
    }

    pub fn menu(
        &self,
        mut menu: PopupMenu,
        entry: DiskEntry,
        header: bool,
        owner: WeakEntity<Self>,
        cx: &App,
    ) -> PopupMenu {
        let busy = self.actions.read(cx).is_busy() || self.resolved_root.is_none();
        let is_root = entry.path == self.root;
        let parent = if entry.directory {
            entry.path.clone()
        } else {
            entry.path.parent().unwrap_or(&self.root).to_owned()
        };
        menu = menu.label(entry.label());
        menu = self.menu_item(
            menu,
            if entry.directory { "展开选中文件夹（仅一层）" } else { "打开文件" },
            if entry.directory {
                TreeCommand::expand(entry.path.clone())
            } else {
                TreeCommand::open(entry.clone())
            },
            is_root,
            owner.clone(),
        );
        menu = menu.separator();
        for (label, command) in [
            ("新建文件…", FileCommand::create_file(self.root.clone(), parent.clone())),
            ("新建文件夹…", FileCommand::create_directory(self.root.clone(), parent.clone())),
        ] {
            menu = self.menu_item(menu, label, TreeCommand::file(command), busy, owner.clone());
        }
        menu = menu.separator();
        for (label, text) in [
            ("复制绝对路径", entry.path.to_string_lossy().into_owned()),
            (
                "复制相对路径",
                if is_root {
                    ".".into()
                } else {
                    entry
                        .path
                        .strip_prefix(&self.root)
                        .unwrap_or(&entry.path)
                        .to_string_lossy()
                        .into_owned()
                },
            ),
            ("复制名称", entry.label()),
        ] {
            menu = self.menu_item(menu, label, TreeCommand::copy(text), false, owner.clone());
        }
        menu = menu.separator();
        menu = self.menu_item(
            menu,
            "重命名…（F2）",
            TreeCommand::file(FileCommand::rename(self.root.clone(), entry.path.clone())),
            busy || is_root,
            owner.clone(),
        );
        menu = self.menu_item(
            menu,
            "永久删除…",
            TreeCommand::file(FileCommand::delete(self.root.clone(), entry.path)),
            busy || is_root,
            owner.clone(),
        );
        if header {
            menu = menu.separator().label("项目");
            for (label, command, disabled) in [
                (
                    "在项目根目录新建文件…",
                    TreeCommand::file(FileCommand::create_file(
                        self.root.clone(),
                        self.root.clone(),
                    )),
                    busy,
                ),
                (
                    "在项目根目录新建文件夹…",
                    TreeCommand::file(FileCommand::create_directory(
                        self.root.clone(),
                        self.root.clone(),
                    )),
                    busy,
                ),
            ] {
                menu = self.menu_item(menu, label, command, disabled, owner.clone());
            }
        }
        menu
    }

    pub fn menu_item(
        &self,
        menu: PopupMenu,
        label: &str,
        command: TreeCommand,
        disabled: bool,
        owner: WeakEntity<Self>,
    ) -> PopupMenu {
        let root = self.root.clone();
        let generation = self.generation;
        menu.item(PopupMenuItem::new(label.to_owned()).disabled(disabled).on_click(
            move |_, window, cx| {
                let owner = owner.clone();
                let root = root.clone();
                let command = command.clone();
                window.defer(cx, move |window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.root == root && this.generation == generation {
                            this.execute_command(command, window, cx);
                        }
                    });
                });
            },
        ))
    }

    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.initialize_focus(window, cx);
        self.focus.focus(window, cx);
    }

    pub fn previous_focus(
        &mut self,
        _: &PreviousFocus,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.tree_focused(window, cx) || !self.keyboard_available(window, cx) {
            cx.propagate();
            return;
        }
        if !self.focus.is_focused(window) {
            self.focus.focus(window, cx);
        }
        window.focus_prev(cx);
    }

    pub fn initialize_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus_subscription.is_some() {
            return;
        }
        self.focus_out_subscription =
            Some(cx.on_focus_out(&self.focus, window, |_, _, _, cx| cx.notify()));
        self.focus_subscription = Some(cx.on_focus(&self.focus, window, |this, window, cx| {
            this.state.update(cx, |state, cx| {
                state.focus(window, cx);
                if state.selected_item().is_none() {
                    let mut ix = 0;
                    while let Some(entry) = state.entry(ix) {
                        if !entry.is_disabled() {
                            state.set_selected_index(Some(ix), cx);
                            break;
                        }
                        ix += 1;
                    }
                }
            });
            cx.notify();
        }));
    }
}

pub fn status_item(path: &Path, label: String) -> TreeItem {
    TreeItem::new(format!("status:{}", path_id(path)), label).disabled(true)
}

pub fn build_items(
    path: &Path,
    directories: &HashMap<PathBuf, DirectoryContents>,
    expanded: &HashSet<PathBuf>,
    paths: &mut HashMap<SharedString, DiskEntry>,
) -> Vec<TreeItem> {
    let Some(contents) = directories.get(path) else {
        return vec![status_item(path, "展开以读取目录".into())];
    };
    let mut items: Vec<TreeItem> = contents
        .entries
        .iter()
        .map(|entry| {
            let id = path_id(&entry.path);
            paths.insert(id.clone(), entry.clone());
            let item = TreeItem::new(id, entry.label());
            if entry.directory {
                let is_expanded = expanded.contains(&entry.path);
                let children = if is_expanded {
                    build_items(&entry.path, directories, expanded, paths)
                } else {
                    vec![status_item(&entry.path, "展开以读取目录".into())]
                };
                item.children(children).expanded(is_expanded)
            } else {
                item
            }
        })
        .collect();
    let status = match &contents.status {
        DirectoryStatus::Unloaded => Some("等待加载目录".into()),
        DirectoryStatus::Loading => Some("正在读取目录…".into()),
        DirectoryStatus::Loaded if items.is_empty() => {
            Some("没有可显示的条目（已过滤忽略项）".into())
        },
        DirectoryStatus::Loaded => None,
        DirectoryStatus::Failed(error) => Some(error.to_string()),
    };
    if let Some(status) = status {
        items.push(status_item(path, status));
    }
    items
}

impl Focusable for FileTree {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FileTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.initialize_focus(window, cx);
        let view = cx.weak_entity();
        let header_view = view.clone();
        let tree_state = self.state.clone();
        let generation = self.generation;
        let root = self.root.clone();
        let loading = !self.tasks.is_empty()
            || self.reveal_task.is_some()
            || self.git_task.is_some()
            || self.search.index_task.is_some()
            || self.search.filter_task.is_some()
            || self.actions.read(cx).is_busy();
        let focused = self.tree_focused(window, cx);
        let error = self
            .reveal_error
            .as_ref()
            .map(ToString::to_string)
            .or_else(|| self.actions.read(cx).error().map(str::to_owned))
            .or_else(|| self.git_error.clone());
        div()
            .size_full().min_w_0().min_h_0().flex().flex_col()
            .key_context("FileTree")
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::find_entry))
            .capture_action(cx.listener(Self::escape_search))
            .child(
                div().flex().items_center().flex_shrink_0().gap_1().px_3().py_1()
                    .child(Icon::new(IconName::FolderTree).small().text_color(cx.theme().muted_foreground))
                    .child(
                        div().id("file-tree-directory").flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD)
                            .child(self.directory.file_name().unwrap_or(self.directory.as_os_str()).to_string_lossy().into_owned())
                            .tooltip({
                                let directory = self.root.display().to_string();
                                move |window, cx| Tooltip::new(directory.clone()).build(window, cx)
                            }),
                    )
                    .when(loading, |header| header.child(Spinner::new().small()))
                    .child(Button::new("search-project-files").ghost().small().icon(Icon::new(IconName::Search).size(rems(0.9375)))
                        .accessibility_label("搜索项目文件").tooltip_with_action("搜索项目文件", &FindEntry::new(), None)
                        .on_click(cx.listener(|this, _, window, cx| this.execute_command(TreeCommand::search(), window, cx))))
                    .child(Button::new("locate-current-file").ghost().small().icon(Icon::new(IconName::LocateFixed).size(rems(0.9375)))
                        .accessibility_label("定位当前文件").tooltip("定位当前文件").disabled(self.current_file.is_none())
                        .on_click(cx.listener(|this, _, window, cx| this.execute_command(TreeCommand::locate(), window, cx))))
                    .child(Button::new("collapse-file-tree").ghost().small().icon(Icon::new(IconName::ListCollapse).size(rems(0.9375)))
                        .accessibility_label("全部折叠").tooltip("全部折叠").disabled(self.expanded.is_empty())
                        .on_click(cx.listener(|this, _, window, cx| this.execute_command(TreeCommand::collapse(), window, cx))))
                    .child(Button::new("refresh-file-tree").ghost().small().icon(Icon::new(IconName::RefreshCw).size(rems(0.9375)))
                        .accessibility_label("刷新目录和 Git 状态").tooltip("刷新目录和 Git 状态")
                        .on_click(cx.listener(|this, _, window, cx| this.execute_command(TreeCommand::refresh(), window, cx))))
                    .child(Button::new("file-tree-more").ghost().small().icon(Icon::new(IconName::Ellipsis).size(rems(0.9375)))
                        .accessibility_label("文件操作及项目菜单").tooltip("文件操作及项目菜单")
                        .dropdown_menu(move |menu, _, cx| {
                            let Some(view) = header_view.upgrade() else { return menu; };
                            let this = view.read(cx);
                            if this.root != root || this.generation != generation { return menu; }
                            let entry = this.selected_entry(cx).unwrap_or_else(|| DiskEntry::new(this.root.clone(), true, false));
                            this.menu(menu, entry, true, header_view.clone(), cx)
                        })),
            )
            .when(self.search.active, |tree| {
                tree.child(div().id("file-tree-search").flex_shrink_0().flex().flex_col().gap_1().px_3().py_2()
                    .capture_action(cx.listener(|this, _: &Search, window, cx| {
                        if this.search_input_focused(window, cx) && this.keyboard_available(window, cx) {
                            this.start_search(None, window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &MoveUp, window, cx| this.navigate_search(true, window, cx)))
                    .capture_action(cx.listener(|this, _: &MoveDown, window, cx| this.navigate_search(false, window, cx)))
                    .capture_action(cx.listener(|this, _: &SelectUp, window, cx| this.navigate_search(true, window, cx)))
                    .capture_action(cx.listener(|this, _: &SelectDown, window, cx| this.navigate_search(false, window, cx)))
                    .child(div().flex().items_center().gap_1()
                        .child(div().flex_1().min_w_0().child(Input::new(&self.search.input).small().aria_label("搜索项目文件名或相对路径")))
                        .child(Button::new("close-file-search").ghost().small().icon(IconName::X).accessibility_label("取消搜索").tooltip("取消搜索（Escape）")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.leave_search(cx);
                                this.focus(window, cx);
                            }))))
                    .child(div().id("file-tree-search-scope").text_xs().text_color(cx.theme().muted_foreground).truncate()
                        .child("项目文件 · 不含忽略目录")
                        .tooltip(|window, cx| Tooltip::new("搜索整个项目的文件名和相对路径；遵循 ignore 规则，不包含忽略目录、.git 或链接目录内的条目。目录读取或规则解析失败会显示错误。").build(window, cx)))
                    .when(self.search.has_query(), |search| search.child(div().text_xs().text_color(cx.theme().muted_foreground)
                        .child(if self.search.truncated {
                            format!("仅显示前 {SEARCH_RESULT_LIMIT} 项，请缩小搜索范围")
                        } else if self.search.index_task.is_some() || self.search.filter_task.is_some() {
                            "正在搜索…".into()
                        } else { format!("{} 项 · ↑↓ 选择 · Enter 打开 · Escape 返回", self.search.results.len()) }))))
            })
            .child(
                div().id("file-tree-content").flex_1().min_w_0().min_h_0()
                    .key_context("FileTreeContent").track_focus(&self.focus)
                    .border_1().border_color(if focused { cx.theme().ring } else { cx.theme().background })
                    .on_key_down(cx.listener(Self::typed_search))
                    .capture_action(cx.listener(|this, _: &SelectLeft, window, cx| {
                        if GlobalState::is_in_deferred_context(cx) && !this.tree_focused(window, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &SelectRight, window, cx| {
                        if GlobalState::is_in_deferred_context(cx) && !this.tree_focused(window, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &SelectUp, window, cx| {
                        if this.tree_focused(window, cx) && this.keyboard_available(window, cx) {
                            this.select_enabled(true, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &SelectDown, window, cx| {
                        if this.tree_focused(window, cx) && this.keyboard_available(window, cx) {
                            this.select_enabled(false, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &Confirm, window, cx| {
                        if this.tree_focused(window, cx) && this.keyboard_available(window, cx) {
                            this.confirm_entry(&ConfirmEntry::new(), window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(Self::confirm_entry))
                    .on_action(cx.listener(Self::previous_focus))
                    .on_action(cx.listener(Self::rename_entry))
                    .on_action(cx.listener(Self::delete_entry))
                    .on_action(cx.listener(Self::copy_relative_path))
                    .child(Tree::new(&self.state, move |_, entry, _, _, cx| {
                        let id = entry.item().id.clone();
                        let label = entry.item().label.clone();
                        let data = view.upgrade().and_then(|view| view.read(cx).paths.get(&id).cloned());
                        let status = data.as_ref().and_then(|data| view.upgrade().and_then(|view| view.read(cx).git_status.status(&data.path)));
                        let color = status.map_or(cx.theme().foreground, |status| status.color(cx));
                        let tooltip = data.as_ref().map_or_else(|| label.to_string(), |data| {
                            format!("{} · {}\n{}", data.kind(), status.map_or("无 Git 变更状态", |status| status.label()), data.path.display())
                        });
                        let icon = if entry.is_disabled() { IconName::Info }
                            else {
                                let icon = data.as_ref().map_or(IconName::File, DiskEntry::icon);
                                if icon == IconName::Folder && entry.is_expanded() { IconName::FolderOpen } else { icon }
                            };
                        let menu_view = view.clone();
                        let select_view = view.clone();
                        let select_id = id.clone();
                        let menu_id = id.clone();
                        let click_view = view.clone();
                        let click_state = tree_state.clone();
                        ListItem::new(id.clone()).h_6().py_0().px_2().text_sm()
                            .accessibility_label(tooltip.clone())
                            .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                            .child(
                                div().id(SharedString::from(format!("file-tree-row:{id}")))
                                    .flex().items_center().gap_1().w_full().min_w_0().pl(rems(entry.depth() as f32))
                                    .on_mouse_down(MouseButton::Right, move |_, window, cx| {
                                        let view = select_view.clone();
                                        let id = select_id.clone();
                                        window.defer(cx, move |_, cx| {
                                            let _ = view.update(cx, |this, cx| {
                                                if this.generation != generation || !this.paths.contains_key(&id) { return; }
                                                this.cancel_reveal();
                                                this.state.update(cx, |state, cx| {
                                                    if let Some(ix) = state.index_of(&id) { state.set_selected_index(Some(ix), cx); }
                                                });
                                                this.remember_selection(cx);
                                                cx.notify();
                                            });
                                        });
                                    })
                                    .child(div().w_4().flex_shrink_0().flex().items_center().justify_center()
                                        .when(entry.is_folder(), |slot| slot.child(Icon::new(if entry.is_expanded() { IconName::ChevronDown } else { IconName::ChevronRight })
                                            .xsmall().text_color(cx.theme().muted_foreground))))
                                    .child(div().size_4().flex_shrink_0().flex().items_center().justify_center().child(Icon::new(icon).small()))
                                    .child(div().flex_1().min_w_0().truncate().text_color(color).child(label))
                                    .child(div().w_4().flex_shrink_0().text_center().text_color(color).child(status.map_or("", |status| status.marker())))
                                    .context_menu(move |menu, _, cx| {
                                        let Some(view) = menu_view.upgrade() else { return menu; };
                                        let this = view.read(cx);
                                        if this.generation != generation { return menu; }
                                        let Some(entry) = this.paths.get(&menu_id).cloned() else { return menu; };
                                        this.menu(menu, entry, false, menu_view.clone(), cx)
                                    }),
                            )
                            .on_click(move |_, window, cx| {
                                let Ok(Some(reveal_id)) = click_view.update(cx, |this, cx| {
                                    if this.generation != generation { return None; }
                                    this.cancel_reveal();
                                    this.reveal_error = None;
                                    cx.notify();
                                    Some(this.reveal_id)
                                }) else { return; };
                                click_state.update(cx, |state, cx| state.focus(window, cx));
                                let view = click_view.clone();
                                let id = id.clone();
                                window.defer(cx, move |window, cx| {
                                    let _ = view.update(cx, |this, cx| this.clicked_entry(&id, generation, reveal_id, window, cx));
                                });
                            })
                    })),
            )
            .when_some(error, |tree, error| tree.child(div().px_3().py_2().text_sm().text_color(cx.theme().danger).child(error)))
    }
}
