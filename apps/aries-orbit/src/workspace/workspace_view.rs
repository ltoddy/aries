use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use agent_client_protocol::AcpAgentConfig;
use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_resizable, resizable_panel};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::terminal::TerminalView;
use crate::workspace::settings::palette::Palette;
use crate::workspace::settings::{Preferences, SettingsState};
use crate::workspace::{SessionRenderMode, SessionView};
use crate::{
    CloseTab, NewSession, NextTab, NextUserMessage, PreviousTab, PreviousUserMessage,
    ToggleTerminal,
};

struct SessionTab {
    id: usize,
    view: Entity<SessionView>,
    _subscription: Subscription,
}

pub struct WorkspaceView {
    cwd: PathBuf,
    settings: Entity<SettingsState>,
    agent: AcpAgentConfig,
    sidebar: Entity<SessionView>,
    _sidebar_subscription: Subscription,
    tabs: Vec<SessionTab>,
    active: usize,
    next_id: usize,
    tab_scroll: ScrollHandle,
    focus_handle: FocusHandle,
    terminal: Option<Entity<TerminalView>>,
    terminal_visible: bool,
    terminal_previous_focus: Option<FocusHandle>,
}

impl fmt::Debug for WorkspaceView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkspaceView")
            .field("cwd", &self.cwd)
            .field("tabs", &self.tabs.len())
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl WorkspaceView {
    pub fn new(
        cwd: PathBuf,
        preferences_path: Option<PathBuf>,
        agent: AcpAgentConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let host = cx.weak_entity();
        let settings = cx.new(|cx| SettingsState::new(preferences_path, host, window, cx));
        let sidebar = Self::create_chat_view(&cwd, &agent, SessionRenderMode::Sidebar, window, cx);
        let sidebar_subscription = Self::observe_tab(&sidebar, window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            cwd,
            settings,
            agent,
            sidebar,
            _sidebar_subscription: sidebar_subscription,
            tabs: Vec::new(),
            active: 0,
            next_id: 0,
            tab_scroll: ScrollHandle::new(),
            focus_handle,
            terminal: None,
            terminal_visible: false,
            terminal_previous_focus: None,
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx.theme().mode);
        let active = self.active_view().cloned();
        let view = active.as_ref().unwrap_or(&self.sidebar);
        let git_info = view.update(cx, |session, cx| session.git_panel().render(view, window, cx));
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.toggle_terminal(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                if let Some(tab) = this.tabs.get(this.active) {
                    let id = tab.id;
                    this.close_tab(id, cx);
                    this.focus_handle.focus(window, cx);
                }
            }))
            .on_action(
                cx.listener(|this, _: &PreviousTab, window, cx| this.switch_tab(false, window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &NextTab, window, cx| this.switch_tab(true, window, cx)),
            )
            .on_action(cx.listener(|this, _: &PreviousUserMessage, _, cx| {
                if let Some(view) = this.active_view() {
                    view.update(cx, |view, cx| view.jump_user_message(false, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &NextUserMessage, _, cx| {
                if let Some(view) = this.active_view() {
                    view.update(cx, |view, cx| view.jump_user_message(true, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &NewSession, window, cx| {
                this.new_session(window, cx);
            }))
            .flex()
            .bg(p.background())
            .text_color(p.foreground())
            .font_family(cx.theme().font_family.clone())
            .text_sm()
            .child(
                h_resizable("layout")
                    .child(
                        resizable_panel()
                            .size(cx.theme().font_size * 18.)
                            .size_range(cx.theme().font_size * 12.5..cx.theme().font_size * 32.5)
                            .child(self.sidebar.clone()),
                    )
                    .child(
                        resizable_panel().child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .h_full()
                                .flex()
                                .flex_col()
                                .child(self.render_tab_bar(p, cx))
                                .child(
                                    div()
                                        .relative()
                                        .flex()
                                        .flex_col()
                                        .flex_1()
                                        .min_h_0()
                                        .child(match active {
                                            Some(view) => {
                                                div().size_full().child(view).into_any_element()
                                            },
                                            None => self.render_empty_main(p),
                                        })
                                        .children(git_info)
                                        .when(self.terminal_visible, |view| {
                                            view.child(self.render_terminal(cx))
                                        }),
                                ),
                        ),
                    ),
            )
    }
}

impl WorkspaceView {
    fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminal_visible {
            self.terminal_visible = false;
            let focus =
                self.terminal_previous_focus.take().unwrap_or_else(|| self.focus_handle.clone());
            window.focus(&focus, cx);
        } else {
            self.terminal_previous_focus = window.focused(cx);
            if self.terminal.is_none() {
                let directory = self
                    .active_view()
                    .map(|view| view.read(cx).execution().directory().to_path_buf())
                    .unwrap_or_else(|| self.cwd.clone());
                self.terminal = Some(cx.new(|cx| TerminalView::new(directory, window, cx)));
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

    fn render_terminal(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let terminal = self.terminal.as_ref().expect("visible terminal has a retained entity");
        let directory = terminal.read(cx).directory().display().to_string();
        div()
            .id("floating-terminal")
            .absolute()
            .inset_0()
            .p_4()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .w_full()
                    .h(relative(0.8))
                    .max_w(rems(64.))
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .bg(cx.theme().background.opacity(0.9))
                    .text_color(cx.theme().foreground)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius)
                    .shadow_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .py_2()
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
                                Button::new("hide-floating-terminal")
                                    .small()
                                    .ghost()
                                    .label("隐藏 ⌘F")
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.toggle_terminal(window, cx)
                                    })),
                            ),
                    )
                    .child(div().flex_1().min_h_0().min_w_0().p_2().child(terminal.clone())),
            )
    }

    pub fn active_view(&self) -> Option<&Entity<SessionView>> {
        self.tabs.get(self.active).map(|tab| &tab.view)
    }

    pub fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active_view().unwrap_or(&self.sidebar).update(cx, |view, cx| {
            view.new_chat_in_current_directory(window, cx);
        });
    }

    pub fn new_tab_in_directory(
        &mut self,
        directory: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.create_tab(directory, window, cx);
    }

    fn create_tab(&mut self, directory: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.next_id;
        self.next_id += 1;
        let view =
            Self::create_chat_view(&self.cwd, &self.agent, SessionRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        self.tabs.push(SessionTab { id, view: view.clone(), _subscription: subscription });
        self.active = self.tabs.len() - 1;
        self.tab_scroll.scroll_to_item(self.active);
        cx.spawn_in(window, async move |_, cx| {
            let _ = view.update_in(cx, |view, window, cx| {
                view.start_new_chat(directory, window, cx);
            });
        })
        .detach();
        cx.notify();
    }

    pub fn open_session(
        &mut self,
        session: SessionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_id = session.session_id.clone();
        if let Some(index) = self.find_session_tab(&session_id, cx) {
            self.select_tab(index, window, cx);
            self.set_sidebar_session(session_id, cx);
            cx.notify();
            return;
        }
        if let Some(tab) = self.tabs.get(self.active)
            && tab.view.read(cx).is_empty_tab(cx)
        {
            tab.view.update(cx, |view, cx| view.queue_session_load(session, cx));
            self.set_sidebar_session(session_id, cx);
            cx.notify();
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let view =
            Self::create_chat_view(&self.cwd, &self.agent, SessionRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        view.update(cx, |view, cx| view.queue_session_load(session, cx));
        self.tabs.push(SessionTab { id, view, _subscription: subscription });
        self.active = self.tabs.len() - 1;
        self.tab_scroll.scroll_to_item(self.active);
        self.set_sidebar_session(session_id, cx);
        cx.notify();
    }

    fn find_session_tab(&self, id: &SessionId, cx: &App) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.view.read(cx).matches_session(id))
    }

    pub fn close_deleted_sessions(&mut self, deleted: &HashSet<SessionId>, cx: &mut Context<Self>) {
        let ids: Vec<_> = self
            .tabs
            .iter()
            .filter(|tab| deleted.iter().any(|id| tab.view.read(cx).matches_session(id)))
            .map(|tab| tab.id)
            .collect();
        for id in ids {
            self.close_tab(id, cx);
        }
    }

    pub fn close_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        self.tabs.remove(index);
        if self.tabs.is_empty() {
            self.active = 0;
        } else if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        } else if index < self.active {
            self.active -= 1;
        }
        self.tab_scroll.scroll_to_item(self.active);
        self.sync_sidebar_active_session(cx);
        cx.notify();
    }

    pub fn select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.active = index;
        self.tab_scroll.scroll_to_item(index);
        self.sync_sidebar_active_session(cx);
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub fn switch_tab(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = adjacent_tab(self.active, self.tabs.len(), next) else {
            return;
        };
        self.select_tab(index, window, cx);
    }
}

fn adjacent_tab(active: usize, count: usize, next: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(if next { (active + 1) % count } else { (active + count - 1) % count })
}

impl WorkspaceView {
    pub fn render_tab_bar(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let can_close = !self.tabs.is_empty();
        div()
            .id("tab-bar")
            .h_9()
            .flex_shrink_0()
            .px_2()
            .gap_1()
            .flex()
            .items_center()
            .bg(p.input())
            .border_b_1()
            .border_color(p.border())
            .child(
                div()
                    .id("tab-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_1()
                    .overflow_x_scroll()
                    .track_scroll(&self.tab_scroll)
                    .children(
                        self.tabs
                            .iter()
                            .enumerate()
                            .map(|(index, tab)| self.render_tab(index, tab, can_close, p, cx)),
                    ),
            )
            .into_any_element()
    }

    pub fn render_empty_main(&self, p: Palette) -> AnyElement {
        div()
            .flex_1()
            .min_h_0()
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
                    .child(div().text_sm().text_color(p.foreground()).child("没有打开的会话"))
                    .child(
                        div()
                            .text_xs()
                            .line_height(rems(1.25))
                            .text_color(p.muted())
                            .child("从左侧选择一个会话，点击“新建会话…”选择项目目录，或按 ⌘N 使用当前目录直接新建会话。"),
                    ),
            )
            .into_any_element()
    }

    fn render_tab(
        &self,
        index: usize,
        tab: &SessionTab,
        can_close: bool,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = tab.id;
        let active = index == self.active;
        let view = tab.view.read(cx);
        let title = view.tab_title();
        let state = view.execution_state();
        let status = state.label();
        let tab_label = format!("切换到标签 {title} · {status}");
        let details = format!("{title}\n{}\n{status}", view.directory().display());
        div()
            .id(("tab", id))
            .h_7()
            .flex_shrink_0()
            .w(rems(11.25))
            .min_w(rems(11.25))
            .px_1()
            .rounded(rems(0.5))
            .border_1()
            .border_color(if active { p.foreground() } else { p.border() })
            .flex()
            .items_center()
            .gap_1()
            .bg(if active { p.background() } else { p.input() })
            .hover(move |style| style.bg(if active { p.background() } else { p.muted() }))
            .child(
                Button::new(("tab-button", id))
                    .ghost()
                    .small()
                    .h_full()
                    .px_0()
                    .flex_1()
                    .min_w_0()
                    .accessibility_label(tab_label)
                    .tooltip(details)
                    .child(state.render_tab_indicator(p))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_tab(index, window, cx);
                    }))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .line_clamp(1)
                            .text_left()
                            .text_xs()
                            .text_color(if active { p.foreground() } else { p.muted() })
                            .child(title),
                    ),
            )
            .when(can_close, |tab| {
                tab.child(
                    Button::new(("close-tab", id))
                        .icon(Icon::new(IconName::X).size(rems(0.6875)))
                        .ghost()
                        .small()
                        .tooltip_with_action("关闭标签", &CloseTab, None)
                        .accessibility_label("关闭标签")
                        .on_click(cx.listener(move |this, _, window, cx| {
                            cx.stop_propagation();
                            this.close_tab(id, cx);
                            this.focus_handle.focus(window, cx);
                        })),
                )
            })
            .into_any_element()
    }
}

impl WorkspaceView {
    pub fn observe_tab(
        view: &Entity<SessionView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.observe_in(view, window, |this, view, window, cx| {
            if view.update(cx, |view, _| view.take_new_tab_request()) {
                this.active_view().unwrap_or(&this.sidebar).update(cx, |view, cx| {
                    view.new_chat(window, cx);
                });
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
        cwd: &Path,
        agent: &AcpAgentConfig,
        mode: SessionRenderMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<SessionView> {
        let host = cx.weak_entity();
        let cwd = cwd.to_path_buf();
        let agent = agent.clone();
        let view = cx.new(|cx| SessionView::new(cwd, agent, window, cx));
        view.update(cx, |view, _| {
            view.set_tab_host(host);
            view.set_render_mode(mode);
        });
        view
    }

    pub fn sync_sidebar_active_session(&mut self, cx: &mut Context<Self>) {
        let snapshot = self.tabs.get(self.active).map(|tab| tab.view.read(cx).sidebar_snapshot());
        self.sidebar.update(cx, |view, cx| view.sync_sidebar_active(snapshot, cx));
    }

    pub fn set_sidebar_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |view, cx| view.set_sidebar_session(session_id, cx));
    }
}

impl WorkspaceView {
    pub fn preferences<'a>(&'a self, cx: &'a App) -> &'a Preferences {
        self.settings.read(cx).preferences()
    }

    pub fn settings_state(&self) -> &Entity<SettingsState> {
        &self.settings
    }

    pub fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings.update(cx, |settings, cx| settings.open_settings(window, cx));
    }

    pub fn notify_settings_sidebar(&self, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |_, cx| cx.notify());
    }

    pub fn notify_settings_tabs(&self, cx: &mut Context<Self>) {
        for tab in &self.tabs {
            tab.view.update(cx, |_, cx| cx.notify());
        }
    }

    pub fn remeasure_timelines(&self, cx: &mut Context<Self>) {
        self.sidebar.read(cx).remeasure_timeline();
        for tab in &self.tabs {
            tab.view.read(cx).remeasure_timeline();
        }
        cx.notify();
    }
}
