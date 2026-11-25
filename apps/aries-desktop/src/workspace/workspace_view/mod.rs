use std::fmt;
use std::path::PathBuf;

use agent_client_protocol::AcpAgentConfig;
use gpui_kit::assets::IconName;
use gpui_kit::component::{ActiveTheme, ResizableState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::extensions::ExtensionsView;
use crate::workspace::files::{FileActionEvent, FileTree, OpenFile};
use crate::workspace::settings::SettingsState;
use crate::workspace::settings::palette::Palette;
use crate::workspace::usage::UsageView;
use crate::workspace::workspace_view::fps_counter::FpsCounter;
use crate::workspace::{SessionRenderMode, SessionView};

mod file_operations;
mod file_tabs;
mod fps_counter;
mod layout;
mod pages;
mod render;
mod session_tabs;
mod sidebar_panel;
mod state;
mod tab;
mod tab_ops;
mod tab_render;

pub use self::layout::{LayoutSizes, SidebarLayout};
pub use self::sidebar_panel::SidebarPanel;
pub use self::state::{FileQueue, SidebarState, TabSet, TerminalState};
pub use self::tab::{DiffTab, FileTab, Tab, TabMeta, WorkspaceTab};

pub struct Shell<'a> {
    pub sizes: &'a LayoutSizes,
    pub sidebar: SidebarLayout,
    pub palette: Palette,
    pub style: StyleRefinement,
}

impl<'a> Shell<'a> {
    pub fn new(
        sizes: &'a LayoutSizes,
        sidebar: SidebarLayout,
        palette: Palette,
        style: StyleRefinement,
    ) -> Self {
        Self { sizes, sidebar, palette, style }
    }
}

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
    file_tree: Entity<FileTree>,
    _file_subscription: Subscription,
    _file_action_subscription: Subscription,
    file_navigation_revision: u64,
    settings: Entity<SettingsState>,
    agent: AcpAgentConfig,
    sidebar_view: Entity<SessionView>,
    _sidebar_subscription: Subscription,
    layout: Entity<ResizableState>,
    sidebar: SidebarState,
    tabs: TabSet,
    files: FileQueue,
    terminal: TerminalState,
    fps_counter: Entity<FpsCounter>,
    focus_handle: FocusHandle,
    usage: Entity<UsageView>,
    extensions: Entity<ExtensionsView>,
}

impl fmt::Debug for WorkspaceView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WorkspaceView")
            .field("cwd", &self.cwd)
            .field("tabs", &self.tabs.items.len())
            .field("active", &self.tabs.active)
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
        let sidebar_view =
            Self::create_chat_view(&cwd, &agent, SessionRenderMode::Sidebar, window, cx);
        let sidebar_subscription = Self::observe_tab(&sidebar_view, window, cx);
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
            file_tree,
            _file_subscription: file_subscription,
            _file_action_subscription: file_action_subscription,
            file_navigation_revision: 0,
            settings,
            agent,
            sidebar_view,
            _sidebar_subscription: sidebar_subscription,
            layout: cx.new(|_| ResizableState::default()),
            sidebar: SidebarState::new(cx),
            tabs: TabSet::new(),
            files: FileQueue::new(),
            terminal: TerminalState::new(cx),
            fps_counter: cx.new(|cx| FpsCounter::new(window, cx)),
            focus_handle,
            usage,
            extensions,
        }
    }
}
