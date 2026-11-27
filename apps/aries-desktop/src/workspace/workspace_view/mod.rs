use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use agent_client_protocol::AcpAgentConfig;
use agent_client_protocol::v1::schema::{SessionId, SessionInfo};
use gpui_kit::assets::IconName;
use gpui_kit::base::GlobalState;
use gpui_kit::base::actions::Confirm;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::dialog::{Dialog, DialogClose, DialogEntrance, DialogFooter};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{
    ActiveTheme, Disableable, Icon, ResizableState, Sizable, TitleBar, WindowExt, h_resizable,
    resizable_panel, v_resizable,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::acp::Event;
use crate::terminal::TerminalView;
use crate::workspace::extensions::ExtensionsView;
use crate::workspace::files::{CloseFile, FileActionEvent, FileTree, FileView, OpenFile, SaveFile};
use crate::workspace::git::diff_view::{DiffTargetChanged, GitDiffView};
use crate::workspace::settings::palette::Palette;
use crate::workspace::settings::{Preferences, SettingsState};
use crate::workspace::usage::UsageView;
use crate::workspace::workspace_view::fps_counter::FpsCounter;
use crate::workspace::{SessionRenderMode, SessionView};
use crate::{
    CloseTab, NewSession, NextTab, NextUserMessage, OpenSettings, PreviousTab, PreviousUserMessage,
    ShowFilesSidebar, ShowGitSidebar, ShowSessionsSidebar, ToggleSidebar, ToggleTerminal,
};

mod file_operations;
mod file_tabs;
mod fps_counter;
mod layout;
mod pages;
mod pane;
mod render;
mod session_tabs;
mod sidebar;
mod tab;
mod tab_ops;
mod tab_render;

pub use self::pane::Pane;
pub use self::tab::{DiffTab, FileTab, Tab, TabMeta, WorkspaceTab};

pub enum FileTabAction {
    Retarget(OpenFile),
    Reload,
    Close,
}

impl FileTabAction {
    fn retarget(target: OpenFile) -> Self {
        Self::Retarget(target)
    }

    fn reload() -> Self {
        Self::Reload
    }

    fn close() -> Self {
        Self::Close
    }
}

#[derive(Clone, Copy)]
pub enum FileTabCommand {
    Search,
    CopyAll,
    Reload,
    CopyAbsolutePath,
    CopyRelativePath,
    Locate,
}

impl FileTabCommand {
    fn search() -> Self {
        Self::Search
    }

    fn copy_all() -> Self {
        Self::CopyAll
    }

    fn reload() -> Self {
        Self::Reload
    }

    fn copy_absolute_path() -> Self {
        Self::CopyAbsolutePath
    }

    fn copy_relative_path() -> Self {
        Self::CopyRelativePath
    }

    fn locate() -> Self {
        Self::Locate
    }
}

#[derive(Clone)]
pub struct DragTab {
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
            .text_sm()
            .shadow_sm()
            .child(div().truncate().child(self.title.clone()))
    }
}

#[derive(Clone, Copy)]
pub enum TabCommand {
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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WorkspacePage {
    Settings,
    Extensions,
    Usage,
}

impl WorkspacePage {
    fn settings() -> Self {
        Self::Settings
    }

    fn extensions() -> Self {
        Self::Extensions
    }

    fn usage() -> Self {
        Self::Usage
    }

    fn title(self) -> &'static str {
        match self {
            Self::Settings => "设置",
            Self::Extensions => "Agent 扩展",
            Self::Usage => "统计",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Settings => IconName::Settings,
            Self::Extensions => IconName::Bot,
            Self::Usage => IconName::ChartNoAxesColumn,
        }
    }

    fn status(self) -> &'static str {
        match self {
            Self::Settings => "偏好设置",
            Self::Extensions => "本地扩展",
            Self::Usage => "用量统计",
        }
    }
}

#[derive(Clone, Copy)]
pub enum PendingFileOperation {
    Close(usize),
    Reload(usize),
    QuitFile(usize),
    Quit,
}

impl PendingFileOperation {
    fn close(id: usize) -> Self {
        Self::Close(id)
    }

    fn reload(id: usize) -> Self {
        Self::Reload(id)
    }

    fn quit_file(id: usize) -> Self {
        Self::QuitFile(id)
    }

    fn quit() -> Self {
        Self::Quit
    }

    fn tab_id(self) -> Option<usize> {
        match self {
            Self::Close(id) | Self::Reload(id) | Self::QuitFile(id) => Some(id),
            Self::Quit => None,
        }
    }
}

pub struct FileConfirmation {
    id: usize,
    saving_requested: bool,
    focus: Option<FocusHandle>,
}

impl FileConfirmation {
    fn new(id: usize, saving_requested: bool) -> Self {
        Self { id, saving_requested, focus: None }
    }
}

pub struct WorkspaceView {
    cwd: PathBuf,
    window: AnyWindowHandle,
    sidebar_pane: Pane,
    file_tree: Entity<FileTree>,
    _file_subscription: Subscription,
    _file_action_subscription: Subscription,
    file_navigation_revision: u64,
    settings: Entity<SettingsState>,
    agent: AcpAgentConfig,
    sidebar: Entity<SessionView>,
    sidebar_visible: bool,
    sidebar_focus_handle: FocusHandle,
    sidebar_toggle_focus_handle: FocusHandle,
    sidebar_sessions_focus_handle: FocusHandle,
    layout: Entity<ResizableState>,
    _sidebar_subscription: Subscription,
    tabs: Vec<WorkspaceTab>,
    pending_file_operations: VecDeque<PendingFileOperation>,
    file_confirmation: Option<FileConfirmation>,
    file_operation_focus: Option<FocusHandle>,
    quit_discarded: HashSet<usize>,
    file_operation_wait: Option<Task<()>>,
    archived_tabs: Vec<Tab<SessionView>>,
    background_tabs: Vec<Tab<SessionView>>,
    active: usize,
    next_id: usize,
    tab_scroll: ScrollHandle,
    fps_counter: Entity<FpsCounter>,
    focus_handle: FocusHandle,
    terminal: Option<Entity<TerminalView>>,
    terminal_visible: bool,
    terminal_zoomed: bool,
    terminal_layout: Entity<ResizableState>,
    terminal_previous_focus: Option<FocusHandle>,
    usage: Entity<UsageView>,
    extensions: Entity<ExtensionsView>,
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
        cwd: impl Into<PathBuf>,
        preferences_path: Option<PathBuf>,
        agent: AcpAgentConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let cwd = cwd.into();
        let host = cx.weak_entity();
        let settings = cx.new(|cx| SettingsState::new(preferences_path, host, window, cx));
        let sidebar = Self::create_chat_view(&cwd, &agent, SessionRenderMode::Sidebar, window, cx);
        let sidebar_subscription = Self::observe_tab(&sidebar, window, cx);
        let file_tree = cx.new(|cx| FileTree::new(cwd.clone(), window, cx));
        let file_subscription =
            cx.subscribe_in(&file_tree, window, |this, _, event: &OpenFile, window, cx| {
                this.open_file(event.clone(), window, cx);
            });
        let file_action_subscription =
            cx.subscribe_in(&file_tree, window, |_, _, event: &FileActionEvent, window, cx| {
                let event = event.clone();
                cx.defer_in(window, move |this, window, cx| {
                    this.on_file_action(event, window, cx);
                });
            });
        let usage = cx.new(|cx| UsageView::new(window, cx));
        let extensions = cx.new(ExtensionsView::new);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            cwd,
            window: window.window_handle(),
            sidebar_pane: Pane::sessions(),
            file_tree,
            _file_subscription: file_subscription,
            _file_action_subscription: file_action_subscription,
            file_navigation_revision: 0,
            settings,
            agent,
            sidebar,
            sidebar_visible: true,
            sidebar_focus_handle: cx.focus_handle(),
            sidebar_toggle_focus_handle: cx.focus_handle().tab_stop(true),
            sidebar_sessions_focus_handle: cx.focus_handle().tab_stop(true),
            layout: cx.new(|_| ResizableState::default()),
            _sidebar_subscription: sidebar_subscription,
            tabs: Vec::new(),
            pending_file_operations: VecDeque::new(),
            file_confirmation: None,
            file_operation_focus: None,
            quit_discarded: HashSet::new(),
            file_operation_wait: None,
            archived_tabs: Vec::new(),
            background_tabs: Vec::new(),
            active: 0,
            next_id: 0,
            tab_scroll: ScrollHandle::new(),
            fps_counter: cx.new(|cx| FpsCounter::new(window, cx)),
            focus_handle,
            terminal: None,
            terminal_visible: false,
            terminal_zoomed: false,
            terminal_layout: cx.new(|_| ResizableState::default()),
            terminal_previous_focus: None,
            usage,
            extensions,
        }
    }
}
