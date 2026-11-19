use crate::workspace::workspace_view::*;

impl WorkspaceView {
    pub fn show_diff(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.active_view().is_some() {
            self.invalidate_file_navigation(cx);
            cx.notify();
        }
    }

    pub fn select_sidebar_page(
        &mut self,
        page: SidebarPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = if page == SidebarPage::files() && self.selected_file_directory(cx).is_none() {
            SidebarPage::sessions()
        } else {
            page
        };
        self.sidebar_page = page;
        self.sidebar_visible = true;
        if page == SidebarPage::files() {
            self.sync_file_directory(cx);
        }
        let view = cx.weak_entity();
        let revision = self.file_navigation_revision;
        let active_id = self.tabs.get(self.active).map(|tab| tab.id);
        window.on_next_frame(move |window, cx| {
            let _ = view.update(cx, |this, cx| {
                if this.sidebar_page != page
                    || !this.sidebar_visible
                    || this.file_navigation_revision != revision
                    || this.tabs.get(this.active).map(|tab| tab.id) != active_id
                {
                    return;
                }
                let detail_open = this
                    .active_view()
                    .is_some_and(|view| view.read(cx).git_panel().diff().read(cx).is_open());
                let minimum = cx.theme().font_size * if detail_open { 42.75 } else { 40.75 };
                if window.viewport_size().width < minimum {
                    this.sidebar_toggle_focus_handle.focus(window, cx);
                } else if page == SidebarPage::files() {
                    this.file_tree.update(cx, |tree, cx| tree.focus(window, cx));
                } else {
                    this.sidebar_sessions_focus_handle.focus(window, cx);
                }
            });
        });
        cx.notify();
    }

    pub fn selected_file_directory(&self, cx: &App) -> Option<PathBuf> {
        if let Some(target) = self.selected_file_view().and_then(|view| view.read(cx).target()) {
            return Some(target.directory().to_owned());
        }
        let execution = self.selected_session_view()?.read(cx).execution();
        (execution.active_session_id().is_some() || execution.directory_selected())
            .then(|| execution.tab_directory().to_owned())
            .filter(|directory| !directory.as_os_str().is_empty())
    }

    pub fn sync_file_directory(&mut self, cx: &mut Context<Self>) {
        let Some(directory) = self.selected_file_directory(cx) else {
            self.file_tree.update(cx, |tree, cx| tree.track_current_file(None, cx));
            if self.sidebar_page == SidebarPage::files() {
                self.sidebar_page = SidebarPage::sessions();
                let owner = cx.weak_entity();
                let window = self.window;
                cx.defer(move |cx| {
                    let _ = window.update(cx, |_, window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if this.selected_file_directory(cx).is_none() && this.sidebar_visible {
                                this.sidebar_sessions_focus_handle.focus(window, cx);
                            }
                        });
                    });
                });
                cx.notify();
            }
            return;
        };
        let current = self
            .active_file_view()
            .and_then(|view| view.read(cx).target())
            .map(|target| target.path().to_owned());
        self.file_tree.update(cx, |tree, cx| {
            tree.set_directory(directory, cx);
            tree.track_current_file(current, cx);
        });
    }

    pub fn render_sidebar(
        &self,
        panel_style: StyleRefinement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let files_available = self.selected_file_directory(cx).is_some();
        let files_selected = files_available && self.sidebar_page == SidebarPage::files();
        let saving =
            self.tabs.iter().filter_map(WorkspaceTab::file).any(|view| view.read(cx).is_saving());
        div()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_1()
                    .child(
                        Button::new("sidebar-sessions")
                            .small()
                            .ghost()
                            .label("会话")
                            .icon(IconName::MessagesSquare)
                            .track_focus(&self.sidebar_sessions_focus_handle)
                            .tooltip_with_action("显示会话列表", &ShowSessionsSidebar::new(), None)
                            .selected(!files_selected)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select_sidebar_page(SidebarPage::sessions(), window, cx);
                            })),
                    )
                    .when(files_available, |tabs| {
                        tabs.child(
                            Button::new("sidebar-files")
                                .small()
                                .ghost()
                                .label("文件")
                                .icon(IconName::FolderTree)
                                .tooltip_with_action("显示文件树", &ShowFilesSidebar::new(), None)
                                .selected(self.sidebar_page == SidebarPage::files())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.select_sidebar_page(SidebarPage::files(), window, cx);
                                })),
                        )
                    }),
            )
            .child(div().flex_1().min_h_0().min_w_0().child(if files_selected {
                div()
                    .size_full()
                    .relative()
                    .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                        if this
                            .tabs
                            .iter()
                            .filter_map(WorkspaceTab::file)
                            .any(|view| view.read(cx).is_saving())
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    }))
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "q"
                            && (event.keystroke.modifiers.platform
                                || event.keystroke.modifiers.control)
                        {
                            return;
                        }
                        if this
                            .tabs
                            .iter()
                            .filter_map(WorkspaceTab::file)
                            .any(|view| view.read(cx).is_saving())
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    }))
                    .child(self.file_tree.clone().cached(panel_style))
                    .when(saving, |tree| {
                        tree.child(
                            div()
                                .absolute()
                                .inset_0()
                                .occlude()
                                .on_any_mouse_down(|_, _, cx| cx.stop_propagation()),
                        )
                    })
                    .into_any_element()
            } else {
                self.sidebar.clone().cached(panel_style).into_any_element()
            }))
            .child(Sidebar::render_footer(
                Some(cx.weak_entity()),
                Palette::new(cx.theme().mode),
                self.is_usage_visible(),
                self.is_extensions_visible(),
                cx,
            ))
            .into_any_element()
    }

    pub fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        if !self.sidebar_visible && self.sidebar_focus_handle.contains_focused(window, cx) {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    pub fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminal_visible {
            self.terminal_visible = false;
            self.terminal_zoomed = false;
            let focus =
                self.terminal_previous_focus.take().unwrap_or_else(|| self.focus_handle.clone());
            window.focus(&focus, cx);
        } else {
            self.terminal_previous_focus = window.focused(cx);
            if self.terminal.is_none() {
                let directory = self.current_directory(cx);
                self.terminal = Some(cx.new(|cx| TerminalView::new(directory, cx)));
            }
            if let Some(terminal) = &self.terminal {
                window.focus(&terminal.read(cx).focus_handle(cx), cx);
            }
            self.terminal_visible = true;
        }
        if let Some(terminal) = &self.terminal {
            terminal.update(cx, |terminal, cx| terminal.set_visible(self.terminal_visible, cx));
        }
        cx.notify();
    }

    pub fn toggle_terminal_zoom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_zoomed = !self.terminal_zoomed;
        if let Some(terminal) = &self.terminal {
            window.focus(&terminal.read(cx).focus_handle(cx), cx);
        }
        cx.notify();
    }

    pub fn render_terminal_layout(
        &self,
        content: AnyElement,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(terminal) = self.terminal.as_ref().filter(|_| self.terminal_visible) else {
            return content;
        };
        let terminal = self.render_terminal(terminal, cx).into_any_element();
        if self.terminal_zoomed {
            return terminal;
        }
        let font_size = cx.theme().font_size;
        let height = (window.viewport_size().height - font_size * 2.625).max(font_size);
        let content_minimum = (font_size * 12.).min(height * 0.6);
        let terminal_minimum = (font_size * 8.).min(height * 0.4);
        let terminal_maximum = height - content_minimum;
        let terminal_height = (height / 3.).clamp(terminal_minimum, terminal_maximum);
        v_resizable("main-terminal-layout")
            .with_state(&self.terminal_layout)
            .child(
                resizable_panel()
                    .size(height - terminal_height)
                    .size_range(content_minimum..Pixels::MAX)
                    .child(content),
            )
            .child(
                resizable_panel()
                    .size(terminal_height)
                    .size_range(terminal_minimum..terminal_maximum)
                    .child(terminal),
            )
            .into_any_element()
    }

    pub fn render_terminal(
        &self,
        terminal: &Entity<TerminalView>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let directory = terminal.read(cx).directory().display().to_string();
        let zoom_label = if self.terminal_zoomed { "还原" } else { "满屏" };
        div()
            .id("terminal-panel")
            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                GlobalState::suppress_text_selection(cx);
            })
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .overflow_hidden()
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child("终端")
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_color(cx.theme().muted_foreground)
                            .child(directory),
                    )
                    .child(
                        Button::new("zoom-terminal")
                            .small()
                            .ghost()
                            .label(zoom_label)
                            .accessibility_label(if self.terminal_zoomed {
                                "还原终端面板"
                            } else {
                                "终端铺满工作区"
                            })
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.toggle_terminal_zoom(window, cx)
                            })),
                    )
                    .child(
                        Button::new("hide-terminal")
                            .small()
                            .ghost()
                            .label("隐藏")
                            .tooltip_with_action("隐藏终端", &ToggleTerminal, None)
                            .on_click(
                                cx.listener(|this, _, window, cx| this.toggle_terminal(window, cx)),
                            ),
                    ),
            )
            .child(div().flex_1().min_h_0().min_w_0().p_2().child(terminal.clone()))
    }
}
