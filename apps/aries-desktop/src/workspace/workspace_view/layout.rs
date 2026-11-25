use crate::workspace::workspace_view::*;

impl WorkspaceView {
    pub fn show_diff(
        &mut self,
        session: &WeakEntity<SessionView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = session.upgrade() else {
            return;
        };
        let diff = session.read(cx).git_panel().diff().clone();
        if !diff.read(cx).is_open() {
            return;
        }
        let existing = self
            .tabs
            .iter()
            .position(|tab| tab.diff().is_some_and(|view| view.entity_id() == diff.entity_id()));
        let index = if let Some(index) = existing {
            index
        } else {
            let id = self.next_id;
            self.next_id += 1;
            let subscription = Self::observe_diff(id, &diff, window, cx);
            let insert = self
                .tabs
                .iter()
                .position(|tab| {
                    tab.session().is_some_and(|view| view.entity_id() == session.entity_id())
                })
                .map(|index| index + 1)
                .unwrap_or(self.tabs.len());
            let tab = Tab::new(id, diff, subscription);
            self.tabs.insert(insert, WorkspaceTab::new_diff(DiffTab::new(tab, session)));
            insert
        };
        self.invalidate_file_navigation(cx);
        self.select_tab(index, window, cx);
        cx.notify();
    }

    pub fn observe_diff(
        id: usize,
        view: &Entity<GitDiffView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(view, window, move |_, view, _: &DiffTargetChanged, window, cx| {
            if view.read(cx).is_open() {
                cx.notify();
                return;
            }
            cx.defer_in(window, move |this, _, cx| {
                if this.tabs.iter().any(|tab| tab.id() == id && !tab.is_pending(cx)) {
                    this.close_tab(id, cx);
                }
            });
        })
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
            if self.sidebar_pane == Pane::files() {
                self.sidebar_pane = Pane::sessions();
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
