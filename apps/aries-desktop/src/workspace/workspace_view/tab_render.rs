use crate::workspace::workspace_view::*;

impl WorkspaceView {
    pub fn render_tab_bar(
        &self,
        p: Palette,
        sidebar_available: Option<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| self.render_tab(index, tab, p, cx))
            .collect();
        let sidebar_visible = self.sidebar_visible && sidebar_available.is_some();
        let label = if sidebar_visible { "收起侧栏" } else { "展开侧栏" };
        let icon = if sidebar_visible { IconName::PanelLeftClose } else { IconName::PanelLeftOpen };
        let shortcut = if cfg!(target_os = "macos") { "⌘B" } else { "Ctrl+B" };
        div()
            .id("tab-bar")
            .h_9()
            .flex_shrink_0()
            .px_2()
            .gap_1()
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .child(
                Button::new("toggle-sidebar")
                    .track_focus(&self.sidebar_toggle_focus_handle)
                    .ghost()
                    .small()
                    .icon(Icon::new(icon))
                    .accessibility_label(label)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .disabled(sidebar_available.is_none())
                    .tooltip(if sidebar_available.is_some() {
                        format!("{label} {shortcut}")
                    } else {
                        "窗口较窄，加宽窗口或收起详情后可展开侧栏".into()
                    })
                    .on_click(|_, window, cx| {
                        cx.stop_propagation();
                        window.dispatch_action(Box::new(ToggleSidebar), cx);
                    }),
            )
            .child(
                div()
                    .id("tab-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_1p5()
                    .overflow_x_scroll()
                    .track_scroll(&self.tab_scroll)
                    .children(tabs),
            )
            .into_any_element()
    }

    pub fn with_tab_interactions<E>(
        &self,
        tab: E,
        id: usize,
        title: SharedString,
        cx: &mut Context<Self>,
    ) -> AnyElement
    where
        E: StatefulInteractiveElement + ParentElement + Styled + IntoElement + 'static,
    {
        let owner = cx.weak_entity();
        let drag_owner = owner.clone();
        let highlight_owner = owner.clone();
        tab.on_drag(DragTab::new(owner.clone(), id, title), |drag, _, _, cx| {
            cx.stop_propagation();
            cx.new(|_| drag.clone())
        })
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
        .can_drop(move |value, _, _| {
            value
                .downcast_ref::<DragTab>()
                .is_some_and(|drag| drag.owner == drag_owner && drag.id != id)
        })
        .drag_over::<DragTab>(move |style, drag, _, cx| {
            if drag.owner == highlight_owner && drag.id != id {
                style.border_color(cx.theme().primary).bg(cx.theme().muted)
            } else {
                style
            }
        })
        .on_drop(cx.listener(move |this, drag: &DragTab, _, cx| this.move_tab(drag, id, cx)))
        .context_menu(move |menu, _, cx| {
            let Some(view) = owner.upgrade() else {
                return menu;
            };
            view.read(cx).tab_menu(menu, id, owner.clone(), cx)
        })
        .into_any_element()
    }

    pub fn tab_menu(
        &self,
        mut menu: PopupMenu,
        id: usize,
        owner: WeakEntity<Self>,
        cx: &App,
    ) -> PopupMenu {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return menu;
        };
        if let Some(view) = tab.file() {
            let ready = view.read(cx).is_ready();
            let open = view.read(cx).is_open();
            let loading = view.read(cx).is_loading();
            let search = if cfg!(target_os = "macos") {
                "搜索文件内容…（⌘F）"
            } else {
                "搜索文件内容…（Ctrl+F）"
            };
            let copy = if cfg!(target_os = "macos") {
                "复制全文（⌘⇧C）"
            } else {
                "复制全文（Ctrl+Shift+C）"
            };
            for (label, command, disabled) in [
                (search, FileTabCommand::search(), !ready),
                (copy, FileTabCommand::copy_all(), !ready),
                ("重新读取文件…", FileTabCommand::reload(), !open || loading),
                ("复制绝对路径", FileTabCommand::copy_absolute_path(), !open),
                ("复制相对路径", FileTabCommand::copy_relative_path(), !open),
                ("在文件树中定位", FileTabCommand::locate(), !open),
            ] {
                let owner = owner.clone();
                menu = menu.item(PopupMenuItem::new(label).disabled(disabled).on_click(
                    move |_, window, cx| {
                        let owner = owner.clone();
                        window.defer(cx, move |window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.execute_file_tab_command(id, command, window, cx);
                            });
                        });
                    },
                ));
            }
            menu = menu.separator();
        }
        let has_others = self.tabs.iter().any(|tab| tab.id != id && !tab.is_pending(cx));
        let has_closable = self.tabs.iter().any(|tab| !tab.is_pending(cx));
        for (label, command, disabled) in [
            ("关闭", TabCommand::close(id), tab.is_pending(cx)),
            ("关闭其他标签", TabCommand::close_others(id), !has_others),
            ("关闭全部标签", TabCommand::close_all(), !has_closable),
        ] {
            let owner = owner.clone();
            menu = menu.item(PopupMenuItem::new(label).disabled(disabled).on_click(
                move |_, window, cx| {
                    window.defer(cx, {
                        let owner = owner.clone();
                        move |window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.execute_tab_command(command, window, cx)
                            });
                        }
                    });
                },
            ));
        }
        menu
    }

    pub fn execute_file_tab_command(
        &mut self,
        id: usize,
        command: FileTabCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let Some(view) = self.tabs[index].file().cloned() else {
            return;
        };
        let Some(target) = view.read(cx).target().cloned() else {
            return;
        };
        match command {
            FileTabCommand::Search => {
                if !view.read(cx).is_ready() {
                    return;
                }
                self.select_tab(index, window, cx);
                if self.tabs.get(self.active).is_none_or(|tab| tab.id != id) {
                    return;
                }
                let revision = self.file_navigation_revision;
                let owner = cx.weak_entity();
                window.on_next_frame(move |window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.is_sessions_visible()
                            && this.file_navigation_revision == revision
                            && this.tabs.get(this.active).is_some_and(|tab| tab.id == id)
                            && let Some(view) = this
                                .tabs
                                .iter()
                                .find(|tab| tab.id == id)
                                .and_then(WorkspaceTab::file)
                            && (this.focus_handle.is_focused(window)
                                || view.read(cx).focus_handle(cx).contains_focused(window, cx))
                        {
                            view.update(cx, |view, cx| view.search(window, cx));
                        }
                    });
                });
            },
            FileTabCommand::CopyAll => view.update(cx, |view, cx| view.copy_all(cx)),
            FileTabCommand::Reload => {
                if !view.read(cx).is_loading() {
                    self.request_file_operations(
                        VecDeque::from([PendingFileOperation::reload(id)]),
                        window,
                        cx,
                    );
                }
            },
            FileTabCommand::CopyAbsolutePath => {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    target.path().to_string_lossy().into_owned(),
                ));
            },
            FileTabCommand::CopyRelativePath => {
                let path = self.normalize_file_path(target.path(), cx);
                let directory = self.normalize_file_path(target.directory(), cx);
                let relative = path.strip_prefix(directory).unwrap_or(&path);
                cx.write_to_clipboard(ClipboardItem::new_string(
                    relative.to_string_lossy().into_owned(),
                ));
            },
            FileTabCommand::Locate => {
                self.select_tab(index, window, cx);
                if self.tabs.get(self.active).is_none_or(|tab| tab.id != id) {
                    return;
                }
                self.file_tree.update(cx, |tree, cx| tree.reveal(target.path(), cx));
                self.select_sidebar_page(SidebarPage::files(), window, cx);
            },
        }
    }

    pub fn file_tab_title(&self, id: usize, target: &OpenFile, cx: &App) -> String {
        let name = target.path().file_name().unwrap_or(target.path().as_os_str());
        let duplicate = self.tabs.iter().any(|tab| {
            tab.id != id
                && tab
                    .file()
                    .and_then(|view| view.read(cx).target())
                    .is_some_and(|target| target.path().file_name() == Some(name))
        });
        if duplicate {
            let parent = target.path().parent().unwrap_or(target.path());
            let directory = self.normalize_file_path(target.directory(), cx);
            let parent = self.normalize_file_path(parent, cx);
            let relative = parent.strip_prefix(&directory).unwrap_or(&parent);
            let relative = relative.to_owned();
            let same_parent = self.tabs.iter().any(|tab| {
                tab.id != id
                    && tab.file().and_then(|view| view.read(cx).target()).is_some_and(|other| {
                        other.path().file_name() == Some(name)
                            && other.path().parent().is_some_and(|parent| {
                                let parent = self.normalize_file_path(parent, cx);
                                let root = self.normalize_file_path(other.directory(), cx);
                                parent.strip_prefix(root).unwrap_or(&parent) == relative
                            })
                    })
            });
            let parent = if same_parent {
                parent.display().to_string()
            } else if relative.as_os_str().is_empty() {
                ".".into()
            } else {
                relative.display().to_string()
            };
            format!("{} — {parent}", name.to_string_lossy())
        } else {
            name.to_string_lossy().into_owned()
        }
    }

    pub fn render_empty_main(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let project = self.cwd.file_name().unwrap_or(self.cwd.as_os_str()).to_string_lossy();
        div()
            .size_full()
            .min_h_0()
            .p_8()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .max_w(rems(22.5))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .text_center()
                    .child(Icon::new(IconName::MessageCircle).size_12().text_color(p.muted()))
                    .child(
                        div()
                            .mt_2()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.foreground())
                            .child(project.into_owned()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(rems(1.25))
                            .text_color(p.muted())
                            .child("从侧栏选择会话，或在当前项目开始新的会话。"),
                    )
                    .child(
                        Button::new("empty-new-session")
                            .small()
                            .mt_4()
                            .icon(IconName::Plus)
                            .label("新建会话")
                            .disabled(!self.sidebar.read(cx).can_start_new_chat())
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(NewSession), cx)
                            }),
                    )
                    .when(cfg!(target_os = "macos"), |empty| {
                        empty.child(
                            div()
                                .mt_2()
                                .text_xs()
                                .text_color(p.muted())
                                .child("⌘N 新建会话 · ⌘B 切换侧栏"),
                        )
                    }),
            )
            .into_any_element()
    }

    pub fn render_tab(
        &self,
        index: usize,
        tab: &WorkspaceTab,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = tab.id;
        let active = index == self.active;
        let pending = tab.is_pending(cx);
        let (title, tooltip, status, indicator) = match &tab.kind {
            WorkspaceTabKind::Session(tab) => {
                let view = tab.view.read(cx);
                let title = view.tab_title();
                let state = view.execution_state();
                (title.clone(), title, state.label(), state.render_tab_indicator(p))
            },
            WorkspaceTabKind::File(tab) => {
                let view = tab.view.read(cx);
                let title = view
                    .target()
                    .map(|target| self.file_tab_title(id, target, cx))
                    .unwrap_or_else(|| "文件".into());
                let tooltip = view
                    .target()
                    .map(|target| target.path().display().to_string())
                    .unwrap_or_else(|| title.clone());
                let loading = view.is_loading();
                let failed = view.has_error() || view.save_error().is_some();
                let indicator = div()
                    .size_4()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if loading {
                        Spinner::new().xsmall().into_any_element()
                    } else {
                        Icon::new(if failed { IconName::Info } else { IconName::FileCode })
                            .size_3()
                            .text_color(if failed { cx.theme().danger } else { p.muted() })
                            .into_any_element()
                    })
                    .into_any_element();
                let status = if loading {
                    "读取中"
                } else if view.save_error().is_some() {
                    "自动保存失败"
                } else if failed {
                    "读取失败"
                } else {
                    "文件"
                };
                (title, tooltip, status, indicator)
            },
            WorkspaceTabKind::Diff(tab) => {
                let view = tab.view.read(cx);
                let target = view.target();
                let title = target
                    .and_then(|target| target.path().file_name())
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "变更".into());
                let tooltip = target
                    .map(|target| {
                        format!("{} · {}", target.group().title(), target.path().display())
                    })
                    .unwrap_or_else(|| title.clone());
                let indicator = div()
                    .size_4()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::FileDiff).size_3().text_color(p.muted()))
                    .into_any_element();
                let status = target.map(|target| target.group().title()).unwrap_or("变更");
                (title, tooltip, status, indicator)
            },
            WorkspaceTabKind::Page(page) => {
                let title = page.title().to_owned();
                let tooltip = title.clone();
                let indicator = div()
                    .size_4()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(page.icon()).size_3().text_color(p.muted()))
                    .into_any_element();
                (title, tooltip, page.status(), indicator)
            },
        };
        let tab_label = format!("切换到标签 {title} · {status}");
        let drag_title = title.clone().into();
        let tab = div()
            .id(("tab", id))
            .h_7()
            .flex_shrink_0()
            .w(rems(11.25))
            .min_w(rems(11.25))
            .pr_0p5()
            .gap_1()
            .rounded(cx.theme().radius_tokens().md)
            .flex()
            .items_center()
            .bg(if active { p.background() } else { p.background().opacity(0.) })
            .when(!active, |tab| tab.hover(|style| style.bg(p.hover())))
            .child(
                Button::new(("tab-button", id))
                    .custom(ButtonCustomVariant::new(cx).active(cx.theme().button_active))
                    .small()
                    .h_full()
                    .px_0()
                    .rounded(cx.theme().radius_tokens().md)
                    .flex_1()
                    .min_w_0()
                    .accessibility_label(tab_label)
                    .tooltip(format!("{tooltip} · {status}"))
                    .child(indicator)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(index) = this.tabs.iter().position(|tab| tab.id == id) {
                            this.select_tab(index, window, cx);
                        }
                    }))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .line_clamp(1)
                            .text_left()
                            .text_sm()
                            .text_color(if active { p.foreground() } else { p.muted() })
                            .child(title),
                    )
                    .when(
                        tab.file().is_some_and(|view| view.read(cx).save_error().is_some()),
                        |button| button.child(div().text_xs().text_color(p.muted()).child(status)),
                    ),
            )
            .child(
                Button::new(("close-tab", id))
                    .icon(Icon::new(IconName::X).size(rems(0.6875)))
                    .ghost()
                    .xsmall()
                    .flex_shrink_0()
                    .text_color(p.muted())
                    .disabled(pending)
                    .tooltip_with_action("关闭标签", &CloseTab, None)
                    .accessibility_label("关闭标签")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.execute_tab_command(TabCommand::close(id), window, cx);
                    })),
            );
        self.with_tab_interactions(tab, id, drag_title, cx)
    }
}
