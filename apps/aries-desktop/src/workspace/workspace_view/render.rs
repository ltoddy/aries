use crate::workspace::workspace_view::*;

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx.theme().mode);
        let font_size = cx.theme().font_size;
        let width = window.viewport_size().width;
        let content_minimum = font_size * 26.;
        let sidebar_minimum = font_size * 13.75;
        let sidebar_maximum = (width - content_minimum - font_size).min(font_size * 30.);
        let sidebar_available = (sidebar_maximum >= sidebar_minimum).then_some(sidebar_maximum);
        let sidebar_visible = self.sidebar_visible && sidebar_available.is_some();
        if !sidebar_visible && self.sidebar_focus_handle.contains_focused(window, cx) {
            cx.defer_in(window, |this, window, cx| {
                if this.sidebar_focus_handle.contains_focused(window, cx) {
                    this.sidebar_toggle_focus_handle.focus(window, cx);
                }
            });
        }
        let sidebar_width = if sidebar_visible {
            self.layout
                .read(cx)
                .sizes()
                .first()
                .copied()
                .unwrap_or(font_size * 16.25)
                .clamp(sidebar_minimum, sidebar_maximum)
        } else {
            Pixels::ZERO
        };
        let panel_style = div().size_full().style().clone();
        let content = self.render_content(p, panel_style.clone(), cx);
        let content = self.render_terminal_layout(content, window, cx);
        let main = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .pr_1p5()
            .pb_1p5()
            .when(!sidebar_visible, |main| main.pl_1p5())
            .child(
                div()
                    .relative()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .rounded(cx.theme().radius_tokens().lg)
                    .bg(p.background())
                    .overflow_hidden()
                    .child(content),
            );
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .capture_action(cx.listener(|this, _: &SaveFile, window, cx| {
                if let Some(id) = this.active_id() {
                    this.save_file_tab(id, window, cx);
                }
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                this.open_settings(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.toggle_terminal(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
                this.toggle_sidebar(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowSessionsSidebar, window, cx| {
                this.select_sidebar_panel(SidebarPanel::sessions(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowFilesSidebar, window, cx| {
                this.select_sidebar_panel(SidebarPanel::files(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &ShowGitSidebar, window, cx| {
                this.select_sidebar_panel(SidebarPanel::git(), window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                if let Some(id) = this.active_id() {
                    this.execute_tab_command(TabCommand::close(id), window, cx);
                }
            }))
            .on_action(
                cx.listener(|this, _: &PreviousTab, window, cx| this.switch_tab(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &NextTab, window, cx| this.switch_tab(true, window, cx)),
            )
            .on_action(cx.listener(|this, _: &PreviousUserMessage, _, cx| {
                if this.is_sessions_visible()
                    && let Some(view) = this.active_view()
                {
                    view.update(cx, |view, cx| view.jump_user_message(false, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &NextUserMessage, _, cx| {
                if this.is_sessions_visible()
                    && let Some(view) = this.active_view()
                {
                    view.update(cx, |view, cx| view.jump_user_message(true, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &NewSession, window, cx| {
                this.new_session(window, cx);
            }))
            .flex()
            .flex_col()
            .bg(p.sidebar().opacity(if cfg!(target_os = "macos") { 0.82 } else { 1. }))
            .text_color(p.foreground())
            .font_family(cx.theme().font_family.clone())
            .text_sm()
            .child(
                TitleBar::new().h_9().bg(p.sidebar().opacity(0.)).border_b_0().child(
                    div()
                        .flex()
                        .items_center()
                        .h_full()
                        .flex_1()
                        .min_w_0()
                        .when(sidebar_visible, |bar| {
                            let inset = if cfg!(target_os = "macos") { px(80.) } else { px(12.) };
                            bar.child(
                                div()
                                    .w(sidebar_width - inset)
                                    .flex_shrink_0()
                                    .px_2()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Aries"),
                            )
                        })
                        .child(self.render_tab_bar(p, sidebar_available, cx))
                        .child(
                            div()
                                .w_20()
                                .h_full()
                                .flex_shrink_0()
                                .child(self.fps_counter.clone().cached(panel_style.clone())),
                        )
                        .child(div().w_3().h_full().flex_shrink_0()),
                ),
            )
            .child(div().flex().items_stretch().flex_1().min_h_0().child(if sidebar_visible {
                h_resizable("layout")
                    .with_state(&self.layout)
                    .child(
                        resizable_panel()
                            .size(cx.theme().font_size * 16.25)
                            .size_range(sidebar_minimum..sidebar_maximum)
                            .child(
                                div()
                                    .size_full()
                                    .track_focus(&self.sidebar_focus_handle)
                                    .child(self.render_sidebar(panel_style, window, cx)),
                            ),
                    )
                    .child(resizable_panel().child(main))
                    .into_any_element()
            } else {
                main.into_any_element()
            }))
    }
}

impl WorkspaceView {
    pub fn render_content(
        &self,
        p: Palette,
        style: StyleRefinement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(tab) = self.active_tab() else {
            return self.render_empty_main(p, cx);
        };
        match tab {
            WorkspaceTab::Page(tab) => {
                let page = self.render_page(tab.page(), style);
                div().size_full().child(page).into_any_element()
            },
            WorkspaceTab::Diff(tab) => {
                if tab.view().read(cx).is_open() {
                    tab.view().clone().cached(style).into_any_element()
                } else {
                    render_detail(tab.session().clone().cached(style))
                }
            },
            WorkspaceTab::File(tab) => render_detail(tab.view().clone().cached(style)),
            WorkspaceTab::Session(tab) => render_detail(tab.view().clone().cached(style)),
        }
    }

    fn render_page(&self, page: WorkspacePage, style: StyleRefinement) -> AnyElement {
        match page {
            WorkspacePage::Settings => self.settings.clone().cached(style).into_any_element(),
            WorkspacePage::Extensions => self.extensions.clone().cached(style).into_any_element(),
            WorkspacePage::Usage => self.usage.clone().cached(style).into_any_element(),
        }
    }

    pub fn observe_tab(
        view: &Entity<SessionView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.observe_in(view, window, |this, view, window, cx| {
            if view.update(cx, |view, _| view.take_new_tab_request()) {
                if this.selected_file_view().is_some() {
                    this.new_session(window, cx);
                } else {
                    this.active_view().unwrap_or(&this.sidebar).update(cx, |view, cx| {
                        view.new_chat(window, cx);
                    });
                }
            } else if let Some(session) = view.update(cx, |view, _| view.take_session_tab_request())
            {
                this.open_session(session, window, cx);
            } else {
                if view != this.sidebar {
                    let created = view.update(cx, |view, _| view.take_created_sidebar_session());
                    if let Some(session) = created {
                        this.sidebar.update(cx, |sidebar, cx| {
                            sidebar.receive_created_sidebar_session(session, cx);
                        });
                    }
                    if this.active_view() == Some(&view) {
                        this.sync_sidebar_active_session(cx);
                    }
                }
                cx.notify();
            }
        })
    }

    pub fn create_chat_view(
        cwd: impl AsRef<Path>,
        agent: &AcpAgentConfig,
        mode: SessionRenderMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SessionView> {
        let host = cx.weak_entity();
        let cwd = cwd.as_ref().to_owned();
        let agent = agent.clone();
        let view = cx.new(|cx| SessionView::new(cwd, agent, window, cx));
        view.update(cx, |view, _| {
            view.set_tab_host(host);
            view.set_render_mode(mode);
        });
        view
    }

    pub fn sync_sidebar_active_session(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.active_view().map(|view| view.read(cx).sidebar_snapshot());
        self.sidebar.update(cx, |view, cx| view.sync_sidebar_active(snapshot, cx));
        self.sync_file_directory(cx);
    }

    pub fn set_sidebar_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |view, cx| view.set_sidebar_session(session_id, cx));
        self.sync_file_directory(cx);
    }
}

fn render_detail(element: impl IntoElement) -> AnyElement {
    div().relative().size_full().min_w_0().min_h_0().child(element).into_any_element()
}
