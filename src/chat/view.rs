mod actions;
mod attachment;
mod commands;
mod composer;
mod events;
mod execution_context;
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

pub(crate) use self::tabs::TabsView;
use crate::acp::{Bridge, Permission};
use crate::chat::model::chat::Chat;
use crate::chat::model::deletion::SessionDeletion;
use crate::chat::model::list::SessionList;
use crate::chat::model::load::SessionLoad;
use crate::chat::view::commands::SlashMenu;
use crate::chat::view::execution_context::GitBranch;
use crate::chat::view::new_chat::NewChatAction;

pub(crate) fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChatRenderMode {
    Full,
    Sidebar,
    Main,
}

pub struct ChatView {
    cwd: PathBuf,
    git_branch: GitBranch,
    git_branch_task: Option<Task<()>>,
    choosing_directory: bool,
    session_directory_selected: bool,
    agent: AcpAgentConfig,
    input: Entity<TextareaState>,
    command_menu: SlashMenu,
    chat: Chat,
    bridge: Option<Bridge>,
    receiver: Option<Task<()>>,
    session_id: Option<SessionId>,
    tab_host: Option<WeakEntity<TabsView>>,
    render_mode: ChatRenderMode,
    queued_session_load: Option<SessionInfo>,
    tab_session_request: Option<SessionInfo>,
    tab_new_request: bool,
    pending_updates: Vec<SessionNotification>,
    drafts: HashMap<SessionId, String>,
    loading_session: Option<SessionLoad>,
    creating_session: Option<NewChatAction>,
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
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("发送消息…")
                .auto_grow(2, 6)
                .submit_on_enter(true)
        });
        let changes = cx.subscribe_in(&input, window, |this, input, event, window, cx| {
            if let InputEvent::PressEnter { shift: false, .. } = event {
                let composing =
                    input.update(cx, |input, cx| input.marked_text_range(window, cx).is_some());
                if !composing && !this.complete_selected_command(window, cx) {
                    this.send(window, cx);
                }
            }
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
            this.watch_git_branch(cx);
            this.connect(window, cx);
        });
        let session_scroll = ScrollHandle::new();
        let current_session_anchor = ScrollAnchor::for_handle(session_scroll.clone());
        Self {
            cwd,
            git_branch: GitBranch::default(),
            git_branch_task: None,
            choosing_directory: false,
            session_directory_selected: false,
            agent,
            input,
            command_menu: SlashMenu::default(),
            chat: Chat::default(),
            bridge: None,
            receiver: None,
            session_id: None,
            tab_host: None,
            render_mode: ChatRenderMode::Full,
            queued_session_load: None,
            tab_session_request: None,
            tab_new_request: false,
            pending_updates: Vec::new(),
            drafts: HashMap::new(),
            loading_session: None,
            creating_session: None,
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
            timeline: ListState::new(0, ListAlignment::Bottom, px(600.)),
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
        let p = palette::Palette::new(cx.theme().mode);
        match self.render_mode {
            ChatRenderMode::Full => self.render_layout(cx).into_any_element(),
            ChatRenderMode::Sidebar => self.sidebar(p, cx).into_any_element(),
            ChatRenderMode::Main => self.render_main_area(p, cx).into_any_element(),
        }
    }
}
