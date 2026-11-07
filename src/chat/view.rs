mod actions;
mod attachment;
mod commands;
mod composer;
mod events;
mod execution_context;
mod git_index;
mod git_info;
mod layout;
mod load_session;
mod media;
mod new_chat;
mod palette;
mod session_actions;
mod session_config;
mod session_list;
mod sidebar;
mod subagent_message;
mod tabs;
mod theme;
mod thought_message;
mod timeline;
mod tool_content;
mod tool_message;
mod user_message;

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use agent_client_protocol::AcpAgentConfig;
use agent_client_protocol::v1::schema::{
    SessionConfigId, SessionId, SessionInfo, SessionNotification,
};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{InputEvent, TextareaState};
use gpui_kit::component::text::TextViewState;
use gpui_kit::prelude::*;
use gpui_kit::*;

pub use self::tabs::TabsView;
use crate::acp::{Bridge, Permission};
use crate::chat::model::chat::Chat;
use crate::chat::model::deletion::SessionDeletion;
use crate::chat::model::list::SessionList;
use crate::chat::model::load::SessionLoad;
use crate::chat::view::commands::SlashMenu;
use crate::chat::view::git_info::GitInfo;
use crate::chat::view::git_info::motion::GitMotion;
use crate::chat::view::sidebar::motion::SidebarMotion;

pub fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChatRenderMode {
    Full,
    Sidebar,
    Main,
}

pub struct ChatView {
    cwd: PathBuf,
    git_info: Option<GitInfo>,
    git_info_task: Option<Task<()>>,
    git_refresh_task: Option<Task<()>>,
    git_refreshing: bool,
    git_refresh_pending: bool,
    git_watch_error: Option<String>,
    git_index_busy: bool,
    git_revision: u64,
    git_info_collapsed: bool,
    git_motion: GitMotion,
    git_pending_index: Option<(PathBuf, git_index::IndexAction)>,
    git_read_error: Option<String>,
    git_trigger_focus: FocusHandle,
    git_panel_focus: FocusHandle,
    choosing_directory: bool,
    session_directory_selected: bool,
    agent: AcpAgentConfig,
    input: Entity<TextareaState>,
    command_menu: SlashMenu,
    chat: Chat,
    bridge: Option<Bridge>,
    receiver: Option<Task<()>>,
    session_id: Option<SessionId>,
    created_sidebar_session: Option<SessionInfo>,
    sidebar_loading_session: Option<SessionId>,
    sidebar_motion: SidebarMotion,
    tab_host: Option<WeakEntity<TabsView>>,
    render_mode: ChatRenderMode,
    queued_session_load: Option<SessionInfo>,
    tab_session_request: Option<SessionInfo>,
    tab_new_request: bool,
    pending_updates: Vec<SessionNotification>,
    drafts: HashMap<SessionId, String>,
    loading_session: Option<SessionLoad>,
    creating_session: bool,
    load_supported: bool,
    session_list: SessionList,
    session_scroll: ScrollHandle,
    current_session_anchor: ScrollAnchor,
    session_deletion: SessionDeletion,
    disconnected: bool,
    stopping: bool,
    status: String,
    tokens: Option<u64>,
    pending_config: Option<SessionConfigId>,
    config_error: Option<String>,
    permissions: VecDeque<Permission>,
    timeline: ListState,
    expanded_thoughts: HashSet<usize>,
    collapsed_streaming_thoughts: HashSet<usize>,
    collapsed_subagents: HashSet<SessionId>,
    tool_diffs: HashMap<(usize, usize), String>,
    images: HashMap<usize, Vec<Arc<Image>>>,
    attachment_images: HashMap<(usize, usize), Arc<Image>>,
    markdown: HashMap<usize, Entity<TextViewState>>,
    _subscriptions: Vec<Subscription>,
}

impl fmt::Debug for ChatView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChatView")
            .field("cwd", &self.cwd)
            .field("session_id", &self.session_id)
            .field("status", &self.status)
            .field("message_count", &self.chat.messages().len())
            .field("disconnected", &self.disconnected)
            .finish_non_exhaustive()
    }
}

impl ChatView {
    pub fn new(
        cwd: PathBuf,
        agent: AcpAgentConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input =
            cx.new(|cx| TextareaState::new(window, cx).placeholder("发送消息…").auto_grow(2, 6));
        let changes = cx.subscribe_in(&input, window, |this, _, event, _, cx| {
            match event {
                InputEvent::Change => this.command_menu.reset(),
                InputEvent::Focus => this.command_menu.focus(),
                InputEvent::Blur => this.command_menu.blur(),
                _ => {}
            }
            cx.notify();
        });
        let quit = cx.on_app_quit(|this, _| {
            this.dismiss_permissions();
            let bridge = this.bridge.take();
            async move {
                if let Some(bridge) = bridge {
                    bridge.shutdown().await;
                }
            }
        });
        let escape = cx.observe_keystrokes(|this, event, _window, cx| {
            if event.keystroke.key == "escape" {
                this.cancel_session_deletion(cx);
            }
        });
        cx.defer_in(window, |this, window, cx| {
            this.watch_git_info(cx);
            this.connect(window, cx);
        });
        let session_scroll = ScrollHandle::new();
        let current_session_anchor = ScrollAnchor::for_handle(session_scroll.clone());
        let timeline = ListState::new(0, ListAlignment::Bottom, px(600.));
        timeline.set_follow_mode(FollowMode::Tail);
        let view = cx.weak_entity();
        timeline.set_scroll_handler(move |_, _, cx| {
            let _ = view.update(cx, |_, cx| cx.notify());
        });
        Self {
            cwd,
            git_info: None,
            git_info_task: None,
            git_refresh_task: None,
            git_refreshing: false,
            git_refresh_pending: false,
            git_watch_error: None,
            git_index_busy: false,
            git_revision: 0,
            git_info_collapsed: true,
            git_motion: GitMotion::default(),
            git_pending_index: None,
            git_read_error: None,
            git_trigger_focus: cx.focus_handle().tab_stop(true),
            git_panel_focus: cx.focus_handle().tab_stop(true),
            choosing_directory: false,
            session_directory_selected: false,
            agent,
            input,
            command_menu: SlashMenu::default(),
            chat: Chat::default(),
            bridge: None,
            receiver: None,
            session_id: None,
            created_sidebar_session: None,
            sidebar_loading_session: None,
            sidebar_motion: SidebarMotion::default(),
            tab_host: None,
            render_mode: ChatRenderMode::Full,
            queued_session_load: None,
            tab_session_request: None,
            tab_new_request: false,
            pending_updates: Vec::new(),
            drafts: HashMap::new(),
            loading_session: None,
            creating_session: false,
            load_supported: false,
            session_list: SessionList::default(),
            session_scroll,
            current_session_anchor,
            session_deletion: SessionDeletion::default(),
            disconnected: false,
            stopping: false,
            status: "就绪".into(),
            tokens: None,
            pending_config: None,
            config_error: None,
            permissions: VecDeque::new(),
            timeline,
            expanded_thoughts: HashSet::new(),
            collapsed_streaming_thoughts: HashSet::new(),
            collapsed_subagents: HashSet::new(),
            tool_diffs: HashMap::new(),
            images: HashMap::new(),
            attachment_images: HashMap::new(),
            markdown: HashMap::new(),
            _subscriptions: vec![changes, quit, escape],
        }
    }
}

impl Render for ChatView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sidebar_motion.cleanup(cx.background_executor().now(), cx.reduce_motion());
        let p = palette::Palette::new(cx.theme().mode);
        match self.render_mode {
            ChatRenderMode::Full => self.render_layout(cx).into_any_element(),
            ChatRenderMode::Sidebar => self.sidebar(p, cx).into_any_element(),
            ChatRenderMode::Main => self.render_main_area(p, cx).into_any_element(),
        }
    }
}
