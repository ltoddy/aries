use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use agent_client_protocol::AcpAgentConfig;
use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::{
    ActiveTheme, Icon, ResizableState, Sizable, h_resizable, resizable_panel,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::terminal::TerminalView;
use crate::workspace::settings::palette::Palette;
use crate::workspace::settings::{Preferences, SettingsState};
use crate::workspace::usage::{ReturnToSessions, UsageView};
use crate::workspace::{SessionRenderMode, SessionView};
use crate::{
    CloseTab, NewSession, NextTab, NextUserMessage, OpenSettings, PreviousTab, PreviousUserMessage,
    ToggleSidebar, ToggleTerminal,
};

struct SessionTab {
    id: usize,
    view: Entity<SessionView>,
    _subscription: Subscription,
}

impl SessionTab {
    fn new(id: usize, view: Entity<SessionView>, subscription: Subscription) -> Self {
        Self { id, view, _subscription: subscription }
    }
}

#[derive(Clone)]
struct DragTab {
    owner: WeakEntity<WorkspaceView>,
    id: usize,
    title: SharedString,
}

impl DragTab {
    fn new(owner: WeakEntity<WorkspaceView>, id: usize, title: SharedString) -> Self {
        Self { owner, id, title }
    }
}

impl Render for DragTab {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(rems(11.25))
            .h_7()
            .px_2()
            .flex()
            .items_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().primary)
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .text_xs()
            .shadow_sm()
            .child(div().truncate().child(self.title.clone()))
    }
}

#[derive(Clone, Copy)]
enum TabCommand {
    Close(usize),
    CloseAll,
    CloseOthers(usize),
}

impl TabCommand {
    fn close(id: usize) -> Self {
        Self::Close(id)
    }

    fn close_all() -> Self {
        Self::CloseAll
    }

    fn close_others(id: usize) -> Self {
        Self::CloseOthers(id)
    }
}

pub struct WorkspaceView {
    cwd: PathBuf,
    settings: Entity<SettingsState>,
    agent: AcpAgentConfig,
    sidebar: Entity<SessionView>,
    sidebar_visible: bool,
    sidebar_focus_handle: FocusHandle,
    sidebar_toggle_focus_handle: FocusHandle,
    layout: Entity<ResizableState>,
    _sidebar_subscription: Subscription,
    tabs: Vec<SessionTab>,
    active: usize,
    next_id: usize,
    tab_scroll: ScrollHandle,
    focus_handle: FocusHandle,
    terminal: Option<Entity<TerminalView>>,
    terminal_visible: bool,
    terminal_previous_focus: Option<FocusHandle>,
    usage: Entity<UsageView>,
    usage_visible: bool,
    _usage_subscription: Subscription,
    usage_previous_focus: Option<FocusHandle>,
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
        let usage = cx.new(|cx| UsageView::new(window, cx));
        let usage_subscription =
            cx.subscribe_in(&usage, window, |this, _, _: &ReturnToSessions, window, cx| {
                this.show_sessions(window, cx);
            });
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            cwd,
            settings,
            agent,
            sidebar,
            sidebar_visible: true,
            sidebar_focus_handle: cx.focus_handle(),
            sidebar_toggle_focus_handle: cx.focus_handle().tab_stop(true),
            layout: cx.new(|_| ResizableState::default()),
            _sidebar_subscription: sidebar_subscription,
            tabs: Vec::new(),
            active: 0,
            next_id: 0,
            tab_scroll: ScrollHandle::new(),
            focus_handle,
            terminal: None,
            terminal_visible: false,
            terminal_previous_focus: None,
            usage,
            usage_visible: false,
            _usage_subscription: usage_subscription,
            usage_previous_focus: None,
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx.theme().mode);
        let active = self.active_view().cloned();
        let view = active.as_ref().unwrap_or(&self.sidebar);
        let git_info = if self.usage_visible {
            None
        } else {
            view.update(cx, |session, cx| session.git_panel().render(view, window, cx))
        };
        let main = div()
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
                    .child(if self.usage_visible {
                        div().size_full().child(self.usage.clone()).into_any_element()
                    } else {
                        match active {
                            Some(view) => div().size_full().child(view).into_any_element(),
                            None => self.render_empty_main(p),
                        }
                    })
                    .children(git_info)
                    .when(self.terminal_visible, |view| view.child(self.render_terminal(cx))),
            );
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &OpenSettings, window, cx| {
                this.open_settings(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleTerminal, window, cx| {
                this.toggle_terminal(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleSidebar, window, cx| {
                this.toggle_sidebar(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                if !this.usage_visible
                    && let Some(tab) = this.tabs.get(this.active)
                {
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
                if !this.usage_visible
                    && let Some(view) = this.active_view()
                {
                    view.update(cx, |view, cx| view.jump_user_message(false, cx));
                }
            }))
            .on_action(cx.listener(|this, _: &NextUserMessage, _, cx| {
                if !this.usage_visible
                    && let Some(view) = this.active_view()
                {
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
            .child(if self.sidebar_visible {
                h_resizable("layout")
                    .with_state(&self.layout)
                    .child(
                        resizable_panel()
                            .size(cx.theme().font_size * 18.)
                            .size_range(cx.theme().font_size * 12.5..cx.theme().font_size * 32.5)
                            .child(
                                div()
                                    .size_full()
                                    .track_focus(&self.sidebar_focus_handle)
                                    .child(self.sidebar.clone()),
                            ),
                    )
                    .child(resizable_panel().child(main))
                    .into_any_element()
            } else {
                main.into_any_element()
            })
    }
}

impl WorkspaceView {
    fn toggle_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sidebar_visible = !self.sidebar_visible;
        if !self.sidebar_visible && self.sidebar_focus_handle.contains_focused(window, cx) {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

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
        if self.usage_visible {
            self.show_sessions(window, cx);
        }
        let id = self.next_id;
        self.next_id += 1;
        let view =
            Self::create_chat_view(&self.cwd, &self.agent, SessionRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        self.tabs.push(SessionTab::new(id, view.clone(), subscription));
        self.usage_visible = false;
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
        if self.usage_visible {
            self.show_sessions(window, cx);
        }
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
        self.tabs.push(SessionTab::new(id, view, subscription));
        self.usage_visible = false;
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

    fn tab_order(&self) -> Vec<usize> {
        self.tabs.iter().map(|tab| tab.id).collect()
    }

    fn move_tab(&mut self, drag: &DragTab, target: usize, cx: &mut Context<Self>) {
        if drag.owner != cx.weak_entity() || drag.id == target {
            return;
        }
        let mut order = self.tab_order();
        let Some(from) = order.iter().position(|id| *id == drag.id) else {
            return;
        };
        let Some(to) = order.iter().position(|id| *id == target) else {
            return;
        };
        let active = self.tabs.get(self.active).map(|tab| tab.id);
        order.remove(from);
        order.insert(to, drag.id);
        self.tabs.sort_by_key(|tab| order.iter().position(|id| *id == tab.id));
        self.active =
            active.and_then(|id| self.tabs.iter().position(|tab| tab.id == id)).unwrap_or(0);
        self.tab_scroll.scroll_to_item(to);
        cx.notify();
    }

    fn execute_tab_command(
        &mut self,
        command: TabCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match command {
            TabCommand::Close(id) => self.close_tab(id, cx),
            TabCommand::CloseAll => {
                self.tabs.clear();
                self.active = 0;
                self.sync_sidebar_active_session(cx);
            },
            TabCommand::CloseOthers(target) => {
                if !self.tab_order().contains(&target) {
                    return;
                }
                self.tabs.retain(|tab| target == tab.id);
                self.active = 0;
                self.sync_sidebar_active_session(cx);
            },
        }
        if !self.usage_visible {
            self.focus_handle.focus(window, cx);
        }
        cx.notify();
    }

    pub fn select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self.usage_visible {
            self.show_sessions(window, cx);
        }
        self.active = index;
        self.tab_scroll.scroll_to_item(index);
        self.sync_sidebar_active_session(cx);
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    pub fn switch_tab(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.usage_visible {
            self.show_sessions(window, cx);
            return;
        }
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
        let tabs: Vec<_> = if self.usage_visible {
            Vec::new()
        } else {
            self.tabs
                .iter()
                .enumerate()
                .map(|(index, tab)| self.render_tab(index, tab, true, p, cx))
                .collect()
        };
        let label = if self.sidebar_visible { "收起会话列表" } else { "展开会话列表" };
        let icon =
            if self.sidebar_visible { IconName::PanelLeftClose } else { IconName::PanelLeftOpen };
        let shortcut = if cfg!(target_os = "macos") { "⌘B" } else { "Ctrl+B" };
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
                Button::new("toggle-sidebar")
                    .track_focus(&self.sidebar_toggle_focus_handle)
                    .ghost()
                    .small()
                    .icon(Icon::new(icon))
                    .accessibility_label(label)
                    .tooltip(format!("{label} {shortcut}"))
                    .on_click(|_, window, cx| window.dispatch_action(Box::new(ToggleSidebar), cx)),
            )
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
                    .children(tabs),
            )
            .into_any_element()
    }

    fn with_tab_interactions<E>(
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
            let has_others = view.read(cx).tab_order().len() > 1;
            Self::tab_menu(menu, id, has_others, owner.clone())
        })
        .into_any_element()
    }

    fn tab_menu(
        mut menu: PopupMenu,
        id: usize,
        has_others: bool,
        owner: WeakEntity<Self>,
    ) -> PopupMenu {
        for (label, command, disabled) in [
            ("关闭", TabCommand::close(id), false),
            ("关闭全部 Tab", TabCommand::close_all(), false),
            ("关闭其他 Tab", TabCommand::close_others(id), !has_others),
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
        let active = index == self.active && !self.usage_visible;
        let view = tab.view.read(cx);
        let title = view.tab_title();
        let state = view.execution_state();
        let status = state.label();
        let tab_label = format!("切换到标签 {title} · {status}");
        let details = format!("{title}\n{}\n{status}", view.directory().display());
        let drag_title = title.clone().into();
        let tab = div()
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
                            this.execute_tab_command(TabCommand::close(id), window, cx);
                        })),
                )
            });
        self.with_tab_interactions(tab, id, drag_title, cx)
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

    pub fn is_usage_visible(&self) -> bool {
        self.usage_visible
    }

    pub fn open_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.usage_visible {
            self.usage_previous_focus = window.focused(cx);
        }
        self.usage_visible = true;
        self.usage.update(cx, |view, cx| view.open(window, cx));
        self.sidebar.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn show_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.usage_visible {
            return;
        }
        self.usage_visible = false;
        let focus = self.usage_previous_focus.take().unwrap_or_else(|| self.focus_handle.clone());
        focus.focus(window, cx);
        self.sidebar.update(cx, |_, cx| cx.notify());
        cx.notify();
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
