use std::collections::HashSet;
use std::fmt;
use std::path::{Path, PathBuf};

use agent_client_protocol::AcpAgentConfig;
use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, Sizable, Theme, h_resizable, resizable_panel,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::new_chat::NewChatAction;
use crate::chat::view::palette::{self, Palette};
use crate::chat::view::{ChatRenderMode, ChatView};
use crate::preferences::{Appearance, Preferences};
use crate::{
    CloseTab, NewSession, NextTab, NextUserMessage, PreviousTab, PreviousUserMessage, ResetZoom,
    ZoomIn, ZoomOut,
};

struct ChatTab {
    id: usize,
    view: Entity<ChatView>,
    _subscription: Subscription,
}

pub struct TabsView {
    cwd: PathBuf,
    pub preferences_path: Option<PathBuf>,
    pub settings: Preferences,
    pub settings_error: Option<String>,
    pub settings_saving: bool,
    save_task: Option<Task<()>>,
    save_revision: usize,
    _settings_subscriptions: Vec<Subscription>,
    agent: AcpAgentConfig,
    sidebar: Entity<ChatView>,
    _sidebar_subscription: Subscription,
    tabs: Vec<ChatTab>,
    active: usize,
    next_id: usize,
    tab_scroll: ScrollHandle,
    focus_handle: FocusHandle,
    base_font_size: Pixels,
    base_mono_font_size: Pixels,
}

impl fmt::Debug for TabsView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TabsView")
            .field("cwd", &self.cwd)
            .field("tabs", &self.tabs.len())
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl TabsView {
    pub fn new(
        cwd: PathBuf,
        preferences_path: Option<PathBuf>,
        agent: AcpAgentConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (settings, settings_error) = match preferences_path.as_deref() {
            Some(path) => Preferences::load_or_migrate(path, Preferences::legacy_path().as_deref()),
            None => (Preferences::default(), Some("未找到用户主目录，设置无法保存".into())),
        };
        let base_font_size = cx.theme().font_size;
        let base_mono_font_size = cx.theme().mono_font_size;
        palette::apply(settings.appearance(), cx);
        Theme::update(cx, |theme| {
            theme.font_size = base_font_size * settings.zoom();
            theme.mono_font_size = base_mono_font_size * settings.zoom();
        });
        let system_theme = cx.observe_window_appearance(window, |this, window, cx| {
            if this.settings.appearance() == Appearance::System {
                palette::sync(window.appearance().into(), cx);
                cx.notify();
            }
        });
        let quit = cx.on_app_quit(|this, _| {
            let task = this.save_task.take();
            async move {
                if let Some(task) = task {
                    task.await;
                }
            }
        });
        let sidebar = Self::create_chat_view(&cwd, &agent, ChatRenderMode::Sidebar, window, cx);
        let sidebar_subscription = Self::observe_tab(&sidebar, window, cx);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            cwd,
            preferences_path,
            settings,
            settings_error,
            settings_saving: false,
            save_task: None,
            save_revision: 0,
            _settings_subscriptions: vec![system_theme, quit],
            agent,
            sidebar,
            _sidebar_subscription: sidebar_subscription,
            tabs: Vec::new(),
            active: 0,
            next_id: 0,
            tab_scroll: ScrollHandle::new(),
            focus_handle,
            base_font_size,
            base_mono_font_size,
        }
    }

    fn observe_tab(
        view: &Entity<ChatView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.observe_in(view, window, |this, view, window, cx| {
            if view.update(cx, |view, _| view.take_new_tab_request()) {
                this.new_session(window, cx);
            } else if let Some(session) = view.update(cx, |view, _| view.take_session_tab_request())
            {
                this.open_session(session, window, cx);
            } else {
                cx.notify();
            }
        })
    }

    fn create_chat_view(
        cwd: &Path,
        agent: &AcpAgentConfig,
        mode: ChatRenderMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ChatView> {
        let host = cx.weak_entity();
        let cwd = cwd.to_path_buf();
        let agent = agent.clone();
        let view = cx.new(|cx| ChatView::new(cwd, agent, window, cx));
        view.update(cx, |view, _| {
            view.set_tab_host(host);
            view.set_render_mode(mode);
        });
        view
    }

    pub fn active_view(&self) -> Option<Entity<ChatView>> {
        self.tabs.get(self.active).map(|tab| tab.view.clone())
    }

    pub fn is_new_session_available(&self, cx: &App) -> bool {
        self.active_view().unwrap_or_else(|| self.sidebar.clone()).read(cx).can_start_new_chat()
    }

    fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let view = self.active_view().unwrap_or_else(|| self.sidebar.clone());
        view.update(cx, |view, cx| {
            if view.session_id.is_some() || view.session_directory_selected {
                view.new_chat_in_directory(view.cwd.clone(), window, cx);
            } else {
                view.new_chat(window, cx);
            }
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
        let view = Self::create_chat_view(&self.cwd, &self.agent, ChatRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        self.tabs.push(ChatTab { id, view: view.clone(), _subscription: subscription });
        self.active = self.tabs.len() - 1;
        self.tab_scroll.scroll_to_item(self.active);
        cx.spawn_in(window, async move |_, cx| {
            let _ = view.update_in(cx, |view, window, cx| {
                view.start_new_chat(directory, NewChatAction::Create, window, cx);
            });
        })
        .detach();
        cx.notify();
    }

    fn sync_sidebar_active_session(&mut self, cx: &mut Context<Self>) {
        let session_id =
            self.tabs.get(self.active).and_then(|tab| tab.view.read(cx).active_session_id());
        self.sidebar.update(cx, |view, _| {
            if let Some(id) = session_id {
                view.track_tab_session(id);
            } else {
                view.untrack_tab_session();
            }
        });
    }

    fn set_sidebar_session(&mut self, session_id: SessionId, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |view, _| {
            view.track_tab_session(session_id);
        });
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
            self.set_sidebar_session(session_id.clone(), cx);
            cx.notify();
            return;
        }
        if let Some(tab) = self.tabs.get(self.active)
            && tab.view.read(cx).is_empty_tab(cx)
        {
            tab.view.update(cx, |view, cx| view.queue_session_load(session, cx));
            self.set_sidebar_session(session_id.clone(), cx);
            cx.notify();
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let view = Self::create_chat_view(&self.cwd, &self.agent, ChatRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        view.update(cx, |view, cx| view.queue_session_load(session, cx));
        self.tabs.push(ChatTab { id, view, _subscription: subscription });
        self.active = self.tabs.len() - 1;
        self.tab_scroll.scroll_to_item(self.active);
        self.set_sidebar_session(session_id.clone(), cx);
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

    fn close_tab(&mut self, id: usize, cx: &mut Context<Self>) {
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

    fn select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        self.active = index;
        self.tab_scroll.scroll_to_item(index);
        self.sync_sidebar_active_session(cx);
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    fn switch_tab(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(index) = adjacent_tab(self.active, self.tabs.len(), next) else {
            return;
        };
        self.select_tab(index, window, cx);
    }

    pub fn zoom(&mut self, step: i32, cx: &mut Context<Self>) {
        let size = zoom_font_size(self.base_font_size, cx.theme().font_size, step);
        Theme::update(cx, |theme| {
            theme.font_size = size;
            theme.mono_font_size = self.base_mono_font_size * (size / self.base_font_size);
        });
        self.settings.set_zoom(size / self.base_font_size);
        self.save_settings(cx);
        self.sidebar.read(cx).timeline.remeasure();
        for tab in &self.tabs {
            tab.view.read(cx).timeline.remeasure();
        }
        cx.notify();
    }

    pub fn set_appearance(&mut self, appearance: Appearance, cx: &mut Context<Self>) {
        self.settings.set_appearance(appearance);
        palette::apply(appearance, cx);
        self.save_settings(cx);
    }

    pub fn save_settings(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.preferences_path.clone() else {
            self.settings_error = Some("设置已生效，但未找到用户主目录，无法保存".into());
            cx.notify();
            return;
        };
        let settings = self.settings;
        self.save_revision += 1;
        let revision = self.save_revision;
        let previous = self.save_task.take();
        self.settings_error = None;
        self.settings_saving = true;
        self.save_task = Some(cx.spawn(async move |view, cx| {
            if let Some(previous) = previous {
                previous.await;
            }
            let result = cx.background_spawn(async move { settings.save(&path) }).await;
            let _ = view.update(cx, |this, cx| {
                if this.save_revision == revision {
                    this.settings_saving = false;
                    this.settings_error =
                        result.err().map(|error| format!("设置已生效，但未保存：{error}"));
                    this.sidebar.update(cx, |_, cx| cx.notify());
                    cx.refresh_windows();
                    cx.notify();
                }
            });
        }));
        self.sidebar.update(cx, |_, cx| cx.notify());
        cx.refresh_windows();
        cx.notify();
    }

    fn render_tab_bar(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
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
            .child(
                Button::new("new-tab")
                    .icon(Icon::new(IconName::Plus).size(rems(0.875)))
                    .ghost()
                    .small()
                    .flex_shrink_0()
                    .tooltip_with_action("新建会话", &NewSession, None)
                    .accessibility_label("新建会话")
                    .disabled(!self.is_new_session_available(cx))
                    .on_click(cx.listener(|this, _, window, cx| this.new_session(window, cx))),
            )
            .into_any_element()
    }

    fn render_empty_main(&self, p: Palette) -> AnyElement {
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
                            .child("从左侧选择一个会话，或点击上方 + 新建会话并选择项目。"),
                    ),
            )
            .into_any_element()
    }

    fn render_tab(
        &self,
        index: usize,
        tab: &ChatTab,
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
        let details = format!("{title}\n{}\n{status}", view.cwd.display());
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

fn adjacent_tab(active: usize, count: usize, next: bool) -> Option<usize> {
    if count == 0 {
        return None;
    }
    Some(if next { (active + 1) % count } else { (active + count - 1) % count })
}

fn zoom_font_size(base: Pixels, current: Pixels, step: i32) -> Pixels {
    if step == 0 {
        base
    } else {
        (current + base * (step as f32 * 0.1)).clamp(base * 0.8, base * 1.5)
    }
}

impl ChatView {
    pub fn set_tab_host(&mut self, tab_host: WeakEntity<TabsView>) {
        self.tab_host = Some(tab_host);
    }

    pub fn set_render_mode(&mut self, render_mode: ChatRenderMode) {
        self.render_mode = render_mode;
    }

    pub fn request_new_tab(&mut self, cx: &mut Context<Self>) {
        self.tab_new_request = true;
        cx.notify();
    }

    fn take_new_tab_request(&mut self) -> bool {
        std::mem::take(&mut self.tab_new_request)
    }

    pub fn request_session_tab(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.tab_session_request = Some(session);
        cx.notify();
    }

    fn take_session_tab_request(&mut self) -> Option<SessionInfo> {
        self.tab_session_request.take()
    }

    pub fn queue_session_load(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.chat.set_title(session.title.clone());
        if self.load_supported && !self.disconnected && self.is_session_idle() {
            self.load_session(session, cx);
        } else {
            self.queued_session_load = Some(session);
            cx.notify();
        }
    }

    pub fn matches_session(&self, id: &SessionId) -> bool {
        self.session_id.as_ref() == Some(id)
            || self.loading_session.as_ref().is_some_and(|loading| loading.session_id() == id)
            || self.queued_session_load.as_ref().is_some_and(|session| &session.session_id == id)
    }

    fn is_empty_tab(&self, cx: &App) -> bool {
        self.session_id.is_none()
            && self.loading_session.is_none()
            && self.queued_session_load.is_none()
            && self.chat.messages().is_empty()
            && self.input.read(cx).value().trim().is_empty()
            && self.is_session_idle()
    }

    pub fn active_session_id(&self) -> Option<SessionId> {
        self.session_id
            .clone()
            .or_else(|| self.loading_session.as_ref().map(|l| l.session_id().clone()))
            .or_else(|| self.queued_session_load.as_ref().map(|s| s.session_id.clone()))
    }

    pub fn tab_title(&self) -> String {
        crate::chat::view::single_line(&self.chat.title())
    }

    pub fn track_tab_session(&mut self, session_id: SessionId) {
        self.session_id = Some(session_id);
    }

    pub fn untrack_tab_session(&mut self) {
        self.session_id = None;
    }
}

impl Render for TabsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx.theme().mode);
        let active = self.active_view();
        let view = active.clone().unwrap_or_else(|| self.sidebar.clone());
        let git_info = crate::chat::view::git_info::render_git_info(view, cx);
        div()
            .size_full()
            .track_focus(&self.focus_handle)
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
            .on_action(cx.listener(|this, _: &ZoomIn, _, cx| this.zoom(1, cx)))
            .on_action(cx.listener(|this, _: &ZoomOut, _, cx| this.zoom(-1, cx)))
            .on_action(cx.listener(|this, _: &ResetZoom, _, cx| this.zoom(0, cx)))
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
                                            }
                                            None => self.render_empty_main(p),
                                        })
                                        .children(git_info),
                                ),
                        ),
                    ),
            )
    }
}
