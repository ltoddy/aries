use std::fmt;
use std::path::{Path, PathBuf};

use agent_client_protocol::AcpAgentConfig;
use agent_client_protocol::schema::v1::{SessionId, SessionInfo};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{ActiveTheme, Icon, Sizable, h_resizable, resizable_panel};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ui::new_chat::NewChatAction;
use crate::ui::palette::Palette;
use crate::ui::{ChatRenderMode, ChatView};

struct ChatTab {
    id: usize,
    view: Entity<ChatView>,
    _subscription: Subscription,
}

pub(crate) struct TabsView {
    cwd: PathBuf,
    preferences_path: Option<PathBuf>,
    agent: AcpAgentConfig,
    sidebar: Entity<ChatView>,
    _sidebar_subscription: Subscription,
    tabs: Vec<ChatTab>,
    active: usize,
    next_id: usize,
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
    pub(crate) fn new(
        cwd: PathBuf,
        preferences_path: Option<PathBuf>,
        agent: AcpAgentConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let sidebar = Self::create_chat_view(
            &cwd,
            preferences_path.clone(),
            &agent,
            ChatRenderMode::Sidebar,
            window,
            cx,
        );
        let sidebar_subscription = Self::observe_tab(&sidebar, window, cx);
        Self {
            cwd,
            preferences_path,
            agent,
            sidebar,
            _sidebar_subscription: sidebar_subscription,
            tabs: Vec::new(),
            active: 0,
            next_id: 0,
        }
    }

    fn observe_tab(
        view: &Entity<ChatView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.observe_in(view, window, |this, view, window, cx| {
            if view.update(cx, |view, _| view.take_new_tab_request()) {
                this.new_tab(window, cx);
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
        preferences_path: Option<PathBuf>,
        agent: &AcpAgentConfig,
        mode: ChatRenderMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ChatView> {
        let host = cx.weak_entity();
        let cwd = cwd.to_path_buf();
        let agent = agent.clone();
        let view = cx.new(|cx| ChatView::new(cwd, preferences_path, agent, window, cx));
        view.update(cx, |view, _| {
            view.set_tab_host(host);
            view.set_render_mode(mode);
        });
        view
    }

    fn active_view(&self) -> Option<Entity<ChatView>> {
        self.tabs.get(self.active).map(|tab| tab.view.clone())
    }

    fn new_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.create_tab(None, window, cx);
    }

    pub(crate) fn new_tab_in_directory(
        &mut self,
        directory: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.create_tab(Some(directory), window, cx);
    }

    fn create_tab(
        &mut self,
        directory: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let id = self.next_id;
        self.next_id += 1;
        let view = Self::create_chat_view(
            &self.cwd,
            self.preferences_path.clone(),
            &self.agent,
            ChatRenderMode::Main,
            window,
            cx,
        );
        let subscription = Self::observe_tab(&view, window, cx);
        self.tabs.push(ChatTab { id, view: view.clone(), _subscription: subscription });
        self.active = self.tabs.len() - 1;
        cx.spawn_in(window, async move |_, cx| {
            let _ = view.update_in(cx, |view, window, cx| {
                if let Some(directory) = directory {
                    view.start_new_chat(directory, NewChatAction::Create, window, cx);
                } else {
                    view.choose_session_directory(NewChatAction::Create, window, cx);
                }
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

    pub(crate) fn open_session(
        &mut self,
        session: SessionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let session_id = session.session_id.clone();
        if let Some(index) = self.find_session_tab(&session_id, cx) {
            self.active = index;
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
        let view = Self::create_chat_view(
            &self.cwd,
            self.preferences_path.clone(),
            &self.agent,
            ChatRenderMode::Main,
            window,
            cx,
        );
        let subscription = Self::observe_tab(&view, window, cx);
        view.update(cx, |view, cx| view.queue_session_load(session, cx));
        self.tabs.push(ChatTab { id, view, _subscription: subscription });
        self.active = self.tabs.len() - 1;
        self.set_sidebar_session(session_id.clone(), cx);
        cx.notify();
    }

    fn find_session_tab(&self, id: &SessionId, cx: &App) -> Option<usize> {
        self.tabs.iter().position(|tab| tab.view.read(cx).matches_session(id))
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
        self.sync_sidebar_active_session(cx);
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
            .children(
                self.tabs
                    .iter()
                    .enumerate()
                    .map(|(index, tab)| self.render_tab(index, tab, can_close, p, cx)),
            )
            .child(
                Button::new("new-tab")
                    .icon(Icon::new(IconName::Plus).size(px(14.)))
                    .ghost()
                    .small()
                    .tooltip("新建标签")
                    .accessibility_label("新建标签")
                    .on_click(cx.listener(|this, _, window, cx| this.new_tab(window, cx))),
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
                    .max_w(px(360.))
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
                            .child("从左侧选择一个会话，或点击上方 + 新建标签。"),
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
        let title = tab.view.read(cx).tab_title();
        let tab_label = format!("切换到标签 {title}");
        div()
            .id(("tab", id))
            .h_7()
            .max_w(px(180.))
            .min_w(px(84.))
            .px_1()
            .rounded(px(8.))
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
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active = index;
                        this.sync_sidebar_active_session(cx);
                        cx.notify();
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
                        .icon(Icon::new(IconName::X).size(px(11.)))
                        .ghost()
                        .small()
                        .tooltip("关闭标签")
                        .accessibility_label("关闭标签")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.stop_propagation();
                            this.close_tab(id, cx);
                        })),
                )
            })
            .into_any_element()
    }
}

impl ChatView {
    pub(crate) fn set_tab_host(&mut self, tab_host: WeakEntity<TabsView>) {
        self.tab_host = Some(tab_host);
    }

    pub(crate) fn set_render_mode(&mut self, render_mode: ChatRenderMode) {
        self.render_mode = render_mode;
    }

    pub(crate) fn request_new_tab(&mut self, cx: &mut Context<Self>) {
        self.tab_new_request = true;
        cx.notify();
    }

    fn take_new_tab_request(&mut self) -> bool {
        std::mem::take(&mut self.tab_new_request)
    }

    pub(crate) fn request_session_tab(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.tab_session_request = Some(session);
        cx.notify();
    }

    fn take_session_tab_request(&mut self) -> Option<SessionInfo> {
        self.tab_session_request.take()
    }

    pub(crate) fn queue_session_load(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.chat.set_title(session.title.clone());
        if self.load_supported && !self.disconnected && self.is_session_idle() {
            self.load_session(session, cx);
        } else {
            self.queued_session_load = Some(session);
            cx.notify();
        }
    }

    pub(crate) fn matches_session(&self, id: &SessionId) -> bool {
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

    pub(crate) fn active_session_id(&self) -> Option<SessionId> {
        self.session_id
            .clone()
            .or_else(|| self.loading_session.as_ref().map(|l| l.session_id().clone()))
            .or_else(|| self.queued_session_load.as_ref().map(|s| s.session_id.clone()))
    }

    pub(crate) fn tab_title(&self) -> String {
        crate::ui::single_line(&self.chat.title())
    }

    pub(crate) fn track_tab_session(&mut self, session_id: SessionId) {
        self.session_id = Some(session_id);
    }

    pub(crate) fn untrack_tab_session(&mut self) {
        self.session_id = None;
    }
}

impl Render for TabsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx.theme().mode);
        let active = self.active_view();
        div()
            .size_full()
            .flex()
            .bg(p.background())
            .text_color(p.foreground())
            .font_family("Monaco")
            .text_sm()
            .child(
                h_resizable("layout")
                    .child(
                        resizable_panel()
                            .size(px(288.))
                            .size_range(px(200.)..px(520.))
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
                                .child(match active {
                                    Some(view) => {
                                        div().flex_1().min_h_0().child(view).into_any_element()
                                    }
                                    None => self.render_empty_main(p),
                                }),
                        ),
                    ),
            )
    }
}
