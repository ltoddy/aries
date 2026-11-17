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
    ActiveTheme, Disableable, Icon, ResizableState, Selectable, Sizable, TitleBar, WindowExt,
    h_resizable, resizable_panel, v_resizable,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::acp::Event;
use crate::terminal::TerminalView;
use crate::workspace::extensions::ExtensionsView;
use crate::workspace::files::{CloseFile, FileActionEvent, FileTree, FileView, OpenFile, SaveFile};
use crate::workspace::settings::palette::Palette;
use crate::workspace::settings::{Preferences, SettingsState};
use crate::workspace::sidebar::Sidebar;
use crate::workspace::usage::{ReturnToSessions, UsageView};
use crate::workspace::{SessionRenderMode, SessionView};
use crate::{
    CloseTab, NewSession, NextTab, NextUserMessage, OpenSettings, PreviousTab, PreviousUserMessage,
    SwitchSidebar, ToggleSidebar, ToggleTerminal,
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

struct FileTab {
    view: Entity<FileView>,
    _subscription: Subscription,
    _close_subscription: Subscription,
    canonical_path: Option<PathBuf>,
    identity_task: Option<Task<()>>,
    pending_target: Option<OpenFile>,
}

impl FileTab {
    fn new(
        view: Entity<FileView>,
        subscription: Subscription,
        close_subscription: Subscription,
    ) -> Self {
        Self {
            view,
            _subscription: subscription,
            _close_subscription: close_subscription,
            canonical_path: None,
            identity_task: None,
            pending_target: None,
        }
    }
}

enum WorkspaceTabKind {
    Session(SessionTab),
    File(FileTab),
}

impl WorkspaceTabKind {
    fn session(tab: SessionTab) -> Self {
        Self::Session(tab)
    }

    fn file(tab: FileTab) -> Self {
        Self::File(tab)
    }
}

struct WorkspaceTab {
    id: usize,
    kind: WorkspaceTabKind,
}

impl WorkspaceTab {
    fn new(id: usize, kind: WorkspaceTabKind) -> Self {
        Self { id, kind }
    }

    fn session(&self) -> Option<&Entity<SessionView>> {
        match &self.kind {
            WorkspaceTabKind::Session(tab) => Some(&tab.view),
            WorkspaceTabKind::File(_) => None,
        }
    }

    fn file(&self) -> Option<&Entity<FileView>> {
        match &self.kind {
            WorkspaceTabKind::Session(_) => None,
            WorkspaceTabKind::File(tab) => Some(&tab.view),
        }
    }

    fn is_pending(&self, cx: &App) -> bool {
        self.session().is_some_and(|view| view.read(cx).management().is_pending())
    }
}

enum FileTabAction {
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
enum FileTabCommand {
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
            .text_sm()
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkspacePage {
    Sessions,
    Usage,
    Extensions,
}

impl WorkspacePage {
    fn sessions() -> Self {
        Self::Sessions
    }

    fn usage() -> Self {
        Self::Usage
    }

    fn extensions() -> Self {
        Self::Extensions
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SidebarPage {
    Sessions,
    Files,
}

impl SidebarPage {
    fn sessions() -> Self {
        Self::Sessions
    }

    fn files() -> Self {
        Self::Files
    }
}

#[derive(Clone, Copy)]
enum PendingFileOperation {
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

struct FileConfirmation {
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
    sidebar_page: SidebarPage,
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
    diff_layout: Entity<ResizableState>,
    _sidebar_subscription: Subscription,
    tabs: Vec<WorkspaceTab>,
    pending_file_operations: VecDeque<PendingFileOperation>,
    file_confirmation: Option<FileConfirmation>,
    file_operation_focus: Option<FocusHandle>,
    quit_discarded: HashSet<usize>,
    file_operation_wait: Option<Task<()>>,
    archived_tabs: Vec<SessionTab>,
    active: usize,
    next_id: usize,
    tab_scroll: ScrollHandle,
    focus_handle: FocusHandle,
    terminal: Option<Entity<TerminalView>>,
    terminal_visible: bool,
    terminal_zoomed: bool,
    terminal_layout: Entity<ResizableState>,
    terminal_previous_focus: Option<FocusHandle>,
    usage: Entity<UsageView>,
    extensions: Entity<ExtensionsView>,
    _extensions_subscription: Subscription,
    page: WorkspacePage,
    _usage_subscription: Subscription,
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
        let usage_subscription =
            cx.subscribe_in(&usage, window, |this, _, _: &ReturnToSessions, window, cx| {
                this.show_sessions(window, cx);
            });
        let extensions = cx.new(ExtensionsView::new);
        let extensions_subscription =
            cx.subscribe_in(&extensions, window, |this, _, _: &ReturnToSessions, window, cx| {
                this.show_sessions(window, cx);
            });
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            cwd,
            window: window.window_handle(),
            sidebar_page: SidebarPage::sessions(),
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
            diff_layout: cx.new(|_| ResizableState::default()),
            _sidebar_subscription: sidebar_subscription,
            tabs: Vec::new(),
            pending_file_operations: VecDeque::new(),
            file_confirmation: None,
            file_operation_focus: None,
            quit_discarded: HashSet::new(),
            file_operation_wait: None,
            archived_tabs: Vec::new(),
            active: 0,
            next_id: 0,
            tab_scroll: ScrollHandle::new(),
            focus_handle,
            terminal: None,
            terminal_visible: false,
            terminal_zoomed: false,
            terminal_layout: cx.new(|_| ResizableState::default()),
            terminal_previous_focus: None,
            usage,
            extensions,
            _extensions_subscription: extensions_subscription,
            page: WorkspacePage::sessions(),
            _usage_subscription: usage_subscription,
        }
    }
}

impl Render for WorkspaceView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx.theme().mode);
        let active = self.active_view().cloned();
        let file = self.active_file_view().cloned();
        let git_info = active.as_ref().and_then(|view| {
            view.update(cx, |session, cx| session.git_panel().render(view, window, cx))
        });
        let diff = active.as_ref().map(|view| view.read(cx).git_panel().diff().clone());
        let diff = diff.filter(|diff| diff.read(cx).is_open());
        let diff_open = diff.is_some();
        let font_size = cx.theme().font_size;
        let width = window.viewport_size().width;
        let content_minimum = font_size * if diff_open { 28. } else { 26. };
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
        let content_width = width - sidebar_width - font_size;
        let panel_style = div().size_full().style().clone();
        let chat = div()
            .relative()
            .size_full()
            .min_w_0()
            .min_h_0()
            .child(if self.is_usage_visible() {
                div()
                    .size_full()
                    .child(self.usage.clone().cached(panel_style.clone()))
                    .into_any_element()
            } else if self.is_extensions_visible() {
                div()
                    .size_full()
                    .child(self.extensions.clone().cached(panel_style.clone()))
                    .into_any_element()
            } else {
                if let Some(file) = file {
                    div().size_full().child(file.cached(panel_style.clone())).into_any_element()
                } else if let Some(view) = active {
                    div().size_full().child(view.cached(panel_style.clone())).into_any_element()
                } else {
                    self.render_empty_main(p, cx)
                }
            })
            .children(git_info);
        let content = if let Some(diff) = diff {
            let detail = diff.cached(panel_style.clone()).into_any_element();
            let diff_width =
                (content_width * 0.6).clamp(font_size * 16., content_width - font_size * 12.);
            h_resizable("chat-diff-layout")
                .with_state(&self.diff_layout)
                .child(resizable_panel().size_range(font_size * 12. ..Pixels::MAX).child(chat))
                .child(
                    resizable_panel()
                        .size(diff_width)
                        .size_range(font_size * 16. ..Pixels::MAX)
                        .child(detail),
                )
                .into_any_element()
        } else {
            chat.into_any_element()
        };
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
                if let Some(id) = this.tabs.get(this.active).map(|tab| tab.id) {
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
            .on_action(cx.listener(|this, _: &SwitchSidebar, window, cx| {
                this.switch_sidebar(window, cx);
            }))
            .on_action(cx.listener(|this, _: &CloseTab, window, cx| {
                if this.is_sessions_visible()
                    && let Some(tab) = this.tabs.get(this.active)
                {
                    let id = tab.id;
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
                                    .child(self.render_sidebar(panel_style, cx)),
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
    pub fn open_file(&mut self, target: OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_confirmation.is_some()
            || !self.pending_file_operations.is_empty()
            || window.has_active_dialog(cx)
        {
            return;
        }
        if !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx)) {
            return;
        }
        let path = self.normalize_file_path(target.path(), cx);
        let existing = self.tabs.iter().position(|tab| {
            let WorkspaceTabKind::File(file) = &tab.kind else {
                return false;
            };
            file.view
                .read(cx)
                .target()
                .is_some_and(|current| self.normalize_file_path(current.path(), cx) == path)
        });
        self.sidebar_page = SidebarPage::files();
        let index = if let Some(index) = existing {
            index
        } else {
            let id = self.next_id;
            self.next_id += 1;
            let view = cx.new(|cx| FileView::new(window, cx));
            let subscription = cx.observe_in(&view, window, move |_, _, window, cx| {
                cx.notify();
                window.refresh();
                cx.defer_in(window, move |this, window, cx| {
                    this.on_file_updated(id, window, cx);
                });
            });
            let close_subscription =
                cx.subscribe_in(&view, window, move |_, _, _: &CloseFile, window, cx| {
                    cx.defer_in(window, move |this, window, cx| {
                        this.execute_tab_command(TabCommand::close(id), window, cx);
                    });
                });
            view.update(cx, |view, cx| view.open(target.clone(), window, cx));
            self.tabs.push(WorkspaceTab::new(
                id,
                WorkspaceTabKind::file(FileTab::new(view, subscription, close_subscription)),
            ));
            self.resolve_file_identity(id, window, cx);
            self.tabs.len() - 1
        };
        let id = self.tabs[index].id;
        self.select_tab(index, window, cx);
        if self.tabs.get(self.active).is_none_or(|tab| tab.id != id) {
            return;
        }
        if let Some(view) = self.active_file_view().cloned() {
            if let Some(line) = target.line().filter(|_| existing.is_some())
                && let Some(current) = view.read(cx).target().cloned()
            {
                let target =
                    OpenFile::new(current.directory().to_owned(), current.path().to_owned())
                        .with_line(line);
                self.reveal_file_tab(id, target, window, cx);
            }
            if let Some(target) = view.read(cx).target().cloned() {
                self.file_tree.update(cx, |tree, cx| tree.reveal(target.path(), cx));
            }
        }
        cx.notify();
    }

    fn reveal_file_tab(
        &self,
        id: usize,
        target: OpenFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let revision = self.file_navigation_revision;
        let owner = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.is_sessions_visible()
                    && this.file_navigation_revision == revision
                    && this.tabs.get(this.active).is_some_and(|tab| tab.id == id)
                    && let Some(view) =
                        this.tabs.iter().find(|tab| tab.id == id).and_then(WorkspaceTab::file)
                    && (this.focus_handle.is_focused(window)
                        || view.read(cx).focus_handle(cx).contains_focused(window, cx))
                {
                    view.update(cx, |view, cx| view.reveal(target, window, cx));
                }
            });
        });
    }

    fn normalize_file_path(&self, path: &Path, cx: &App) -> PathBuf {
        self.file_tree.read(cx).normalize_path(path)
    }

    fn resolve_file_identity(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) else {
            return;
        };
        let WorkspaceTabKind::File(file) = &mut tab.kind else {
            return;
        };
        file.identity_task = None;
        file.canonical_path = None;
        let Some(target) = file.view.read(cx).target() else {
            return;
        };
        let path = target.path().to_owned();
        file.identity_task = Some(cx.spawn_in(window, async move |this, cx| {
            let request = path.clone();
            let canonical =
                cx.background_spawn(async move { std::fs::canonicalize(request).ok() }).await;
            let _ = this.update_in(cx, |this, window, cx| {
                let Some(tab) = this.tabs.iter_mut().find(|tab| tab.id == id) else {
                    return;
                };
                let WorkspaceTabKind::File(file) = &mut tab.kind else {
                    return;
                };
                if file.view.read(cx).target().is_none_or(|target| target.path() != path) {
                    return;
                }
                file.identity_task = None;
                file.canonical_path = canonical.clone();
                let Some(canonical) = canonical else {
                    return;
                };
                let mut duplicates: Vec<_> = this
                    .tabs
                    .iter()
                    .filter_map(|tab| match &tab.kind {
                        WorkspaceTabKind::File(file)
                            if file.canonical_path.as_ref() == Some(&canonical) =>
                        {
                            Some(tab.id)
                        },
                        _ => None,
                    })
                    .collect();
                if !this.pending_file_operations.is_empty() {
                    cx.notify();
                    return;
                }
                duplicates.sort_unstable();
                let protected: Vec<_> = this
                    .tabs
                    .iter()
                    .filter(|tab| duplicates.contains(&tab.id))
                    .filter(|tab| {
                        tab.file().is_some_and(|view| {
                            view.read(cx).is_dirty() || view.read(cx).is_saving()
                        })
                    })
                    .map(|tab| tab.id)
                    .collect();
                let Some(keep) = protected.first().or(duplicates.first()).copied() else {
                    return;
                };
                let remove: Vec<_> = duplicates
                    .into_iter()
                    .filter(|id| *id != keep && !protected.contains(id))
                    .collect();
                if remove.is_empty() {
                    return;
                }
                let active_id = this.tabs.get(this.active).map(|tab| tab.id);
                let replaced = active_id.is_some_and(|id| remove.contains(&id));
                let focused = this.active_file_view().is_some_and(|view| {
                    view.read(cx).focus_handle(cx).contains_focused(window, cx)
                }) || this.focus_handle.is_focused(window);
                let line = this
                    .selected_file_view()
                    .and_then(|view| view.read(cx).target())
                    .and_then(OpenFile::line);
                this.tabs.retain(|tab| !remove.contains(&tab.id));
                let active_id = if replaced { Some(keep) } else { active_id };
                this.active = active_id
                    .and_then(|id| this.tabs.iter().position(|tab| tab.id == id))
                    .unwrap_or(0);
                if replaced {
                    this.invalidate_file_navigation(cx);
                    this.sync_sidebar_active_session(cx);
                    this.tab_scroll.scroll_to_item(this.active);
                    if focused {
                        this.focus_active_tab(window, cx);
                        if let Some(line) = line
                            && let Some(view) = this.active_file_view()
                            && let Some(current) = view.read(cx).target().cloned()
                        {
                            let target = OpenFile::new(
                                current.directory().to_owned(),
                                current.path().to_owned(),
                            )
                            .with_line(line);
                            this.reveal_file_tab(keep, target, window, cx);
                        }
                    }
                }
                cx.notify();
            });
        }));
    }

    fn on_file_action(
        &mut self,
        event: FileActionEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let actions: Vec<_> = {
            let tree = self.file_tree.read(cx);
            let event = match event {
                FileActionEvent::Renamed { from, to } => {
                    FileActionEvent::renamed(tree.normalize_path(&from), tree.normalize_path(&to))
                },
                FileActionEvent::Deleted(path) => {
                    FileActionEvent::deleted(tree.normalize_path(&path))
                },
                FileActionEvent::Refresh(path) => {
                    FileActionEvent::refresh(tree.normalize_path(&path))
                },
                FileActionEvent::Created(path) => {
                    FileActionEvent::created(tree.normalize_path(&path))
                },
            };
            self.tabs
                .iter()
                .filter_map(|tab| {
                    let WorkspaceTabKind::File(file) = &tab.kind else {
                        return None;
                    };
                    let target =
                        file.pending_target.as_ref().or_else(|| file.view.read(cx).target())?;
                    let primary = tree.normalize_path(target.path());
                    let canonical =
                        file.canonical_path.as_deref().map(|path| tree.normalize_path(path));
                    let action = match &event {
                        FileActionEvent::Renamed { from, to } => {
                            if let Ok(relative) = primary.strip_prefix(from) {
                                let directory = tree.normalize_path(target.directory());
                                let directory = directory
                                    .strip_prefix(from)
                                    .map(|relative| {
                                        if relative.as_os_str().is_empty() {
                                            to.clone()
                                        } else {
                                            to.join(relative)
                                        }
                                    })
                                    .unwrap_or_else(|_| target.directory().to_owned());
                                let path = if relative.as_os_str().is_empty() {
                                    to.clone()
                                } else {
                                    to.join(relative)
                                };
                                let mut updated = OpenFile::new(directory, path);
                                if let Some(line) = target.line() {
                                    updated = updated.with_line(line);
                                }
                                FileTabAction::retarget(updated)
                            } else if canonical.as_ref().is_some_and(|path| path.starts_with(from))
                            {
                                FileTabAction::retarget(target.clone())
                            } else {
                                return None;
                            }
                        },
                        FileActionEvent::Deleted(path) => {
                            if primary.starts_with(path)
                                || canonical
                                    .as_ref()
                                    .is_some_and(|canonical| canonical.starts_with(path))
                            {
                                FileTabAction::close()
                            } else {
                                return None;
                            }
                        },
                        FileActionEvent::Refresh(path) => {
                            if primary.starts_with(path)
                                || canonical
                                    .as_ref()
                                    .is_some_and(|canonical| canonical.starts_with(path))
                            {
                                FileTabAction::reload()
                            } else {
                                return None;
                            }
                        },
                        FileActionEvent::Created(_) => return None,
                    };
                    Some((tab.id, action))
                })
                .collect()
        };
        for (id, action) in actions {
            match action {
                FileTabAction::Retarget(target) => self.retarget_file_tab(id, target, window, cx),
                FileTabAction::Reload => self.refresh_file_tab(id, window, cx),
                FileTabAction::Close => self.close_tab(id, cx),
            }
        }
        self.sync_file_directory(cx);
        cx.notify();
    }

    fn file_view(&self, id: usize) -> Option<&Entity<FileView>> {
        self.tabs.iter().find(|tab| tab.id == id).and_then(WorkspaceTab::file)
    }

    fn refresh_file_tab(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(view) = self.file_view(id).cloned() else {
            return;
        };
        if view.read(cx).is_dirty() || view.read(cx).is_saving() {
            return;
        }
        let focus = window.focused(cx);
        view.update(cx, |view, cx| view.reload(window, cx));
        if let Some(focus) = focus {
            focus.focus(window, cx);
        }
        self.resolve_file_identity(id, window, cx);
    }

    fn retarget_file_tab(
        &mut self,
        id: usize,
        target: OpenFile,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab) = self.tabs.iter_mut().find(|tab| tab.id == id) else {
            return;
        };
        let WorkspaceTabKind::File(file) = &mut tab.kind else {
            return;
        };
        file.identity_task = None;
        file.canonical_path = None;
        if file.view.read(cx).is_saving() {
            file.pending_target = Some(target);
            return;
        }
        file.pending_target = None;
        let focus = window.focused(cx);
        file.view.update(cx, |view, cx| view.retarget(target, window, cx));
        if let Some(focus) = focus {
            focus.focus(window, cx);
        }
        self.resolve_file_identity(id, window, cx);
    }

    fn save_file_tab(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_confirmation.is_some()
            || !self.pending_file_operations.is_empty()
            || window.has_active_dialog(cx)
        {
            return;
        }
        if !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx)) {
            return;
        }
        if let Some(view) = self.file_view(id) {
            view.update(cx, |view, cx| view.save(window, cx));
        }
    }

    fn on_file_updated(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.tabs.iter_mut().find_map(|tab| {
            if tab.id != id {
                return None;
            }
            let WorkspaceTabKind::File(file) = &mut tab.kind else {
                return None;
            };
            (!file.view.read(cx).is_saving()).then(|| file.pending_target.take()).flatten()
        });
        if let Some(target) = target {
            self.retarget_file_tab(id, target, window, cx);
            self.sync_file_directory(cx);
        }
        if self
            .file_confirmation
            .as_ref()
            .is_some_and(|confirmation| confirmation.id == id && confirmation.saving_requested)
            && let Some(view) = self.file_view(id).cloned()
            && !view.read(cx).is_saving()
        {
            if view.read(cx).save_error().is_none() {
                if view.read(cx).is_dirty() {
                    view.update(cx, |view, cx| view.save(window, cx));
                } else {
                    self.finish_file_confirmation(id, window, cx);
                    return;
                }
            } else if let Some(confirmation) = &mut self.file_confirmation {
                confirmation.saving_requested = false;
                window.refresh();
                cx.notify();
            }
        }
        if self.file_confirmation.is_none() && !self.pending_file_operations.is_empty() {
            self.advance_file_operations(window, cx);
        }
    }

    pub fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_confirmation.is_some() || !self.pending_file_operations.is_empty() {
            return;
        }
        self.file_operation_focus = window.focused(cx);
        self.quit_discarded.clear();
        self.pending_file_operations.push_back(PendingFileOperation::quit());
        self.pause_file_operation_auto_save(cx);
        self.advance_file_operations(window, cx);
    }

    fn request_file_operations(
        &mut self,
        operations: VecDeque<PendingFileOperation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.pending_file_operations.is_empty()
            || self.file_confirmation.is_some()
            || window.has_active_dialog(cx)
        {
            return;
        }
        self.file_operation_focus = window.focused(cx);
        self.pending_file_operations = operations;
        self.pause_file_operation_auto_save(cx);
        self.advance_file_operations(window, cx);
    }

    fn pause_file_operation_auto_save(&self, cx: &mut Context<Self>) {
        let quitting = self
            .pending_file_operations
            .iter()
            .any(|operation| matches!(operation, PendingFileOperation::Quit));
        for tab in &self.tabs {
            if (quitting
                || self
                    .pending_file_operations
                    .iter()
                    .any(|operation| operation.tab_id() == Some(tab.id)))
                && let Some(view) = tab.file()
            {
                view.update(cx, |view, _| view.pause_auto_save());
            }
        }
    }

    fn resume_file_auto_save(&self, window: &mut Window, cx: &mut Context<Self>) {
        for view in self.tabs.iter().filter_map(WorkspaceTab::file) {
            view.update(cx, |view, cx| view.resume_auto_save(window, cx));
        }
    }

    fn advance_file_operations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_confirmation.is_some() {
            return;
        }
        while let Some(operation) = self.pending_file_operations.front().copied() {
            if matches!(operation, PendingFileOperation::Quit)
                && self.tabs.iter().all(|tab| {
                    self.quit_discarded.contains(&tab.id)
                        || tab.file().is_none_or(|view| {
                            !view.read(cx).is_dirty() && !view.read(cx).is_saving()
                        })
                })
            {
                self.pending_file_operations.clear();
                self.file_operation_wait = None;
                cx.quit();
                return;
            }
            if window.has_active_dialog(cx)
                || !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx))
            {
                self.wait_for_file_operations(window, cx);
                return;
            }
            self.file_operation_wait = None;
            if let Some(id) = operation.tab_id() {
                if let Some(view) = self.file_view(id).cloned() {
                    view.update(cx, |view, _| view.pause_auto_save());
                    if view.read(cx).is_saving() {
                        self.wait_for_file_operations(window, cx);
                        return;
                    }
                    let reload = matches!(operation, PendingFileOperation::Reload(_));
                    if !reload && view.read(cx).is_dirty() && view.read(cx).save_error().is_none() {
                        view.update(cx, |view, cx| view.save(window, cx));
                        self.wait_for_file_operations(window, cx);
                        return;
                    }
                    if !reload && !view.read(cx).is_dirty() {
                        self.pending_file_operations.pop_front();
                        self.apply_file_operation(operation, window, cx);
                        continue;
                    }
                    self.file_confirmation = Some(FileConfirmation::new(id, false));
                    self.invalidate_file_navigation(cx);
                    let owner = cx.weak_entity();
                    window.open_dialog(cx, move |dialog, _, cx| {
                        let Some(entity) = owner.upgrade() else {
                            return dialog.on_ok(|_, _, _| false);
                        };
                        entity.read(cx).render_file_confirmation(dialog, id, owner.clone(), cx)
                    });
                    if let Some(confirmation) = &mut self.file_confirmation {
                        confirmation.focus = window.focused(cx);
                    }
                    cx.notify();
                    return;
                }
                self.pending_file_operations.pop_front();
                self.apply_file_operation(operation, window, cx);
            } else {
                let dirty = self.tabs.iter().find(|tab| {
                    !self.quit_discarded.contains(&tab.id)
                        && tab.file().is_some_and(|view| {
                            view.read(cx).is_dirty() || view.read(cx).is_saving()
                        })
                });
                if let Some(tab) = dirty {
                    self.pending_file_operations
                        .push_front(PendingFileOperation::quit_file(tab.id));
                }
            }
        }
        self.file_operation_focus = None;
        self.quit_discarded.clear();
        self.file_operation_wait = None;
        self.resume_file_auto_save(window, cx);
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    fn wait_for_file_operations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_operation_wait.is_some() {
            return;
        }
        self.file_operation_wait = Some(cx.spawn_in(window, async move |owner, cx| {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let _ = owner.update_in(cx, |this, window, cx| {
                this.file_operation_wait = None;
                this.advance_file_operations(window, cx);
            });
        }));
    }

    fn apply_file_operation(
        &mut self,
        operation: PendingFileOperation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match operation {
            PendingFileOperation::Close(id) => self.remove_tab(id, cx),
            PendingFileOperation::Reload(id) => {
                if let Some(view) = self.file_view(id) {
                    view.update(cx, |view, cx| {
                        view.reload_discarding(window, cx);
                        view.resume_auto_save(window, cx);
                    });
                }
                self.resolve_file_identity(id, window, cx);
            },
            PendingFileOperation::QuitFile(_) => {},
            PendingFileOperation::Quit => {},
        }
    }

    fn confirmation_is_busy(&self, id: usize, cx: &App) -> bool {
        self.file_confirmation
            .as_ref()
            .is_none_or(|confirmation| confirmation.id != id || confirmation.saving_requested)
            || self.file_view(id).is_some_and(|view| view.read(cx).is_saving())
    }

    fn save_file_confirmation(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.confirmation_is_busy(id, cx) {
            return;
        }
        let Some(view) = self.file_view(id).cloned() else {
            return;
        };
        if matches!(self.pending_file_operations.front(), Some(PendingFileOperation::Reload(_)))
            && (!view.read(cx).is_dirty() || view.read(cx).save_error().is_none())
        {
            self.discard_file_confirmation(id, window, cx);
            return;
        }
        if !view.read(cx).is_ready()
            || !view.read(cx).is_dirty()
            || view.read(cx).save_error().is_none()
        {
            return;
        }
        view.update(cx, |view, cx| {
            view.pause_auto_save();
            view.save(window, cx);
        });
        if let Some(confirmation) = &mut self.file_confirmation {
            confirmation.saving_requested = view.read(cx).is_saving();
        }
        window.refresh();
        cx.notify();
    }

    fn owns_file_confirmation(&self, id: usize, window: &Window, cx: &App) -> bool {
        self.file_confirmation.as_ref().is_some_and(|confirmation| {
            confirmation.id == id
                && confirmation.focus.as_ref().is_some_and(|focus| {
                    focus.is_focused(window) || focus.contains_focused(window, cx)
                })
        })
    }

    fn discard_file_confirmation(
        &mut self,
        id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.confirmation_is_busy(id, cx) || !self.owns_file_confirmation(id, window, cx) {
            return;
        }
        if matches!(self.pending_file_operations.front(), Some(PendingFileOperation::QuitFile(_))) {
            self.quit_discarded.insert(id);
        }
        self.finish_file_confirmation(id, window, cx);
    }

    fn finish_file_confirmation(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.owns_file_confirmation(id, window, cx) {
            return;
        }
        self.file_confirmation = None;
        window.close_dialog(cx);
        if let Some(operation) = self.pending_file_operations.pop_front() {
            self.apply_file_operation(operation, window, cx);
        }
        cx.defer_in(window, |this, window, cx| this.advance_file_operations(window, cx));
        cx.notify();
    }

    fn cancel_file_confirmation(
        &mut self,
        id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.confirmation_is_busy(id, cx) {
            return false;
        }
        self.file_confirmation = None;
        self.pending_file_operations.clear();
        self.quit_discarded.clear();
        self.file_operation_wait = None;
        cx.defer_in(window, |this, window, cx| {
            if this.pending_file_operations.is_empty() {
                this.resume_file_auto_save(window, cx);
            }
        });
        if let Some(focus) = self.file_operation_focus.take() {
            window.defer(cx, move |window, cx| focus.focus(window, cx));
        }
        cx.notify();
        true
    }

    fn render_file_confirmation(
        &self,
        dialog: Dialog,
        id: usize,
        owner: WeakEntity<Self>,
        cx: &App,
    ) -> Dialog {
        let busy = self.confirmation_is_busy(id, cx);
        let view = self.file_view(id).map(|view| view.read(cx));
        let target = view.and_then(FileView::target);
        let name = target.map(|target| {
            target.path().file_name().unwrap_or(target.path().as_os_str()).to_string_lossy()
        });
        let reload =
            matches!(self.pending_file_operations.front(), Some(PendingFileOperation::Reload(_)));
        let title = if reload {
            format!("重新读取“{}”？", name.unwrap_or_default())
        } else if busy {
            format!("正在保存“{}”", name.unwrap_or_default())
        } else {
            format!("“{}”自动保存失败", name.unwrap_or_default())
        };
        let save = owner.clone();
        let cancel = owner.clone();
        let discard = owner.clone();
        let enter = owner.clone();
        let confirm = owner.clone();
        let dirty = view.is_some_and(FileView::is_dirty);
        let retry = busy || view.is_some_and(|view| view.is_dirty() && view.save_error().is_some());
        let disabled_save = busy
            || view.is_none_or(|view| {
                if reload && !retry { !view.is_open() } else { !view.is_ready() || !dirty }
            });
        dialog
            .title(title)
            .w(cx.theme().font_size * 28.)
            .entrance(DialogEntrance::None)
            .overlay_closable(false)
            .close_button(false)
            .on_ok(move |_, window, cx| {
                let _ = save.update(cx, |this, cx| this.save_file_confirmation(id, window, cx));
                false
            })
            .on_cancel(move |_, window, cx| {
                cancel
                    .update(cx, |this, cx| this.cancel_file_confirmation(id, window, cx))
                    .unwrap_or(false)
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .whitespace_normal()
                            .child(
                                target
                                    .map(|target| target.path().display().to_string())
                                    .unwrap_or_default(),
                            ),
                    )
                    .child(if reload {
                        "自动保存已暂停。重新读取磁盘内容将丢弃未保存的修改，无法撤销。"
                    } else if busy {
                        "正在重试保存，草稿仍保留在当前标签中。"
                    } else {
                        "自动保存失败，草稿仍保留在当前标签中。可以重试保存；放弃修改后无法恢复。"
                    })
                    .when(busy, |body| {
                        body.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(Spinner::new().small())
                                .child("正在保存，请稍候"),
                        )
                    })
                    .when_some(view.and_then(FileView::save_error), |body, error| {
                        body.child(
                            div().text_sm().text_color(cx.theme().danger).child(error.to_owned()),
                        )
                    }),
            )
            .footer(
                div()
                    .capture_key_down(move |event, window, cx| {
                        if event.keystroke.key == "enter"
                            && event.keystroke.modifiers == Modifiers::default()
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                            let _ = enter
                                .update(cx, |this, cx| this.save_file_confirmation(id, window, cx));
                        }
                    })
                    .capture_action(move |_: &Confirm, window, cx| {
                        cx.stop_propagation();
                        let _ = confirm
                            .update(cx, |this, cx| this.save_file_confirmation(id, window, cx));
                    })
                    .child(
                        DialogFooter::new()
                            .when(dirty && (!reload || retry), |footer| {
                                footer.child(
                                    Button::new(("discard-file-changes", id))
                                        .danger()
                                        .label("放弃修改")
                                        .disabled(busy)
                                        .on_click(move |_, window, cx| {
                                            let _ = discard.update(cx, |this, cx| {
                                                this.discard_file_confirmation(id, window, cx);
                                            });
                                        }),
                                )
                            })
                            .child(div().flex_shrink_0().child(DialogClose::new().trigger(
                                |button| {
                                    button.label("取消").accessibility_label("取消").disabled(busy)
                                },
                            )))
                            .when(retry || reload, |footer| {
                                footer.child(
                                    Button::new(("save-file-changes", id))
                                        .primary()
                                        .when(reload && !retry && dirty, |button| button.danger())
                                        .label(if retry {
                                            "重试保存"
                                        } else if dirty {
                                            "放弃修改"
                                        } else {
                                            "重新读取"
                                        })
                                        .disabled(disabled_save)
                                        .on_click(move |_, window, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.save_file_confirmation(id, window, cx)
                                            });
                                        }),
                                )
                            }),
                    ),
            )
    }

    pub fn file_navigation_revision(&self) -> u64 {
        self.file_navigation_revision
    }

    pub fn is_active_session(&self, owner: &WeakEntity<SessionView>) -> bool {
        self.active_view().is_some_and(|view| owner.upgrade().as_ref() == Some(view))
    }

    fn invalidate_file_navigation(&mut self, _: &mut Context<Self>) {
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
    }

    pub fn show_diff(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.active_view().is_some() {
            self.invalidate_file_navigation(cx);
            cx.notify();
        }
    }

    fn switch_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page = if self.sidebar_page == SidebarPage::sessions() {
            SidebarPage::files()
        } else {
            SidebarPage::sessions()
        };
        self.select_sidebar_page(page, window, cx);
    }

    fn select_sidebar_page(
        &mut self,
        page: SidebarPage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = if page == SidebarPage::files() && self.selected_file_directory(cx).is_none() {
            SidebarPage::sessions()
        } else {
            page
        };
        self.sidebar_page = page;
        self.sidebar_visible = true;
        if page == SidebarPage::files() {
            self.sync_file_directory(cx);
        }
        let view = cx.weak_entity();
        let revision = self.file_navigation_revision;
        let active_id = self.tabs.get(self.active).map(|tab| tab.id);
        window.on_next_frame(move |window, cx| {
            let _ = view.update(cx, |this, cx| {
                if this.sidebar_page != page
                    || !this.sidebar_visible
                    || this.file_navigation_revision != revision
                    || this.tabs.get(this.active).map(|tab| tab.id) != active_id
                {
                    return;
                }
                let detail_open = this
                    .active_view()
                    .is_some_and(|view| view.read(cx).git_panel().diff().read(cx).is_open());
                let minimum = cx.theme().font_size * if detail_open { 42.75 } else { 40.75 };
                if window.viewport_size().width < minimum {
                    this.sidebar_toggle_focus_handle.focus(window, cx);
                } else if page == SidebarPage::files() {
                    this.file_tree.update(cx, |tree, cx| tree.focus(window, cx));
                } else {
                    this.sidebar_sessions_focus_handle.focus(window, cx);
                }
            });
        });
        cx.notify();
    }

    fn selected_file_directory(&self, cx: &App) -> Option<PathBuf> {
        if let Some(target) = self.selected_file_view().and_then(|view| view.read(cx).target()) {
            return Some(target.directory().to_owned());
        }
        let execution = self.selected_session_view()?.read(cx).execution();
        (execution.active_session_id().is_some() || execution.directory_selected())
            .then(|| execution.tab_directory().to_owned())
            .filter(|directory| !directory.as_os_str().is_empty())
    }

    fn sync_file_directory(&mut self, cx: &mut Context<Self>) {
        let Some(directory) = self.selected_file_directory(cx) else {
            self.file_tree.update(cx, |tree, cx| tree.track_current_file(None, cx));
            if self.sidebar_page == SidebarPage::files() {
                self.sidebar_page = SidebarPage::sessions();
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

    fn render_sidebar(&self, panel_style: StyleRefinement, cx: &mut Context<Self>) -> AnyElement {
        let files_available = self.selected_file_directory(cx).is_some();
        let files_selected = files_available && self.sidebar_page == SidebarPage::files();
        let saving =
            self.tabs.iter().filter_map(WorkspaceTab::file).any(|view| view.read(cx).is_saving());
        div()
            .size_full()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_2()
                    .py_1()
                    .child(
                        Button::new("sidebar-sessions")
                            .small()
                            .ghost()
                            .label("会话")
                            .icon(IconName::MessagesSquare)
                            .track_focus(&self.sidebar_sessions_focus_handle)
                            .tooltip_with_action("切换会话 / 文件", &SwitchSidebar::new(), None)
                            .selected(!files_selected)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.select_sidebar_page(SidebarPage::sessions(), window, cx);
                            })),
                    )
                    .when(files_available, |tabs| {
                        tabs.child(
                            Button::new("sidebar-files")
                                .small()
                                .ghost()
                                .label("文件")
                                .icon(IconName::FolderTree)
                                .tooltip_with_action("切换会话 / 文件", &SwitchSidebar::new(), None)
                                .selected(self.sidebar_page == SidebarPage::files())
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.select_sidebar_page(SidebarPage::files(), window, cx);
                                })),
                        )
                    }),
            )
            .child(div().flex_1().min_h_0().min_w_0().child(if files_selected {
                div()
                    .size_full()
                    .relative()
                    .capture_any_mouse_down(cx.listener(|this, _, window, cx| {
                        if this
                            .tabs
                            .iter()
                            .filter_map(WorkspaceTab::file)
                            .any(|view| view.read(cx).is_saving())
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    }))
                    .capture_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "q"
                            && (event.keystroke.modifiers.platform
                                || event.keystroke.modifiers.control)
                        {
                            return;
                        }
                        if this
                            .tabs
                            .iter()
                            .filter_map(WorkspaceTab::file)
                            .any(|view| view.read(cx).is_saving())
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                        }
                    }))
                    .child(self.file_tree.clone().cached(panel_style))
                    .when(saving, |tree| {
                        tree.child(
                            div()
                                .absolute()
                                .inset_0()
                                .occlude()
                                .on_any_mouse_down(|_, _, cx| cx.stop_propagation()),
                        )
                    })
                    .into_any_element()
            } else {
                self.sidebar.clone().cached(panel_style).into_any_element()
            }))
            .child(Sidebar::render_footer(
                Some(cx.weak_entity()),
                Palette::new(cx.theme().mode),
                self.is_usage_visible(),
                self.is_extensions_visible(),
                cx,
            ))
            .into_any_element()
    }

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

    fn toggle_terminal_zoom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal_zoomed = !self.terminal_zoomed;
        if let Some(terminal) = &self.terminal {
            window.focus(&terminal.read(cx).focus_handle(cx), cx);
        }
        cx.notify();
    }

    fn render_terminal_layout(
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

    fn render_terminal(
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

    fn selected_session_view(&self) -> Option<&Entity<SessionView>> {
        self.tabs.get(self.active).and_then(WorkspaceTab::session)
    }

    fn selected_file_view(&self) -> Option<&Entity<FileView>> {
        self.tabs.get(self.active).and_then(WorkspaceTab::file)
    }

    pub fn active_view(&self) -> Option<&Entity<SessionView>> {
        self.is_sessions_visible().then(|| self.selected_session_view()).flatten()
    }

    fn active_file_view(&self) -> Option<&Entity<FileView>> {
        self.is_sessions_visible().then(|| self.selected_file_view()).flatten()
    }

    fn current_directory(&self, cx: &App) -> PathBuf {
        self.selected_file_directory(cx).unwrap_or_else(|| self.cwd.clone())
    }

    pub fn new_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(view) = self.selected_session_view() {
            view.update(cx, |view, cx| view.new_chat_in_current_directory(window, cx));
        } else {
            let directory = self.current_directory(cx);
            self.sidebar.update(cx, |view, cx| view.new_chat_in_directory(directory, window, cx));
        }
    }

    pub fn new_tab_in_directory(
        &mut self,
        directory: impl Into<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.create_tab(directory.into(), window, cx);
    }

    fn create_tab(&mut self, directory: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if !self.pending_file_operations.is_empty() {
            return;
        }
        if !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx)) {
            return;
        }
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
        if !self.is_sessions_visible() {
            self.show_sessions(window, cx);
        }
        let id = self.next_id;
        self.next_id += 1;
        let view =
            Self::create_chat_view(&directory, &self.agent, SessionRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        self.tabs.push(WorkspaceTab::new(
            id,
            WorkspaceTabKind::session(SessionTab::new(id, view.clone(), subscription)),
        ));
        self.file_tree.update(cx, |tree, cx| tree.set_directory(directory.clone(), cx));
        self.page = WorkspacePage::sessions();
        self.active = self.tabs.len() - 1;
        self.tab_scroll.scroll_to_item(self.active);
        self.sync_sidebar_active_session(cx);
        self.focus_active_tab(window, cx);
        let view = view.downgrade();
        cx.spawn_in(window, async move |_, cx| {
            let _ = cx.update(|window, cx| {
                let _ = view.update(cx, |view, cx| view.start_new_chat(directory, window, cx));
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
        if session.archived.unwrap_or(false)
            || self.sidebar.read(cx).management().is_pending()
            || !self.pending_file_operations.is_empty()
        {
            return;
        }
        if !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx)) {
            return;
        }
        if !self.is_sessions_visible() {
            self.show_sessions(window, cx);
        }
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
        let session_id = session.session_id.clone();
        if let Some(index) =
            self.archived_tabs.iter().position(|tab| tab.view.read(cx).matches_session(&session_id))
        {
            let tab = self.archived_tabs.remove(index);
            tab.view.update(cx, |view, cx| view.reload_archived_session(session, window, cx));
            self.tabs.push(WorkspaceTab::new(tab.id, WorkspaceTabKind::session(tab)));
            self.select_tab(self.tabs.len() - 1, window, cx);
            return;
        }
        if let Some(index) = self.find_session_tab(&session_id, cx) {
            self.select_tab(index, window, cx);
            self.set_sidebar_session(session_id, cx);
            cx.notify();
            return;
        }
        if let Some(view) = self.active_view()
            && view.read(cx).is_empty_tab(cx)
        {
            view.update(cx, |view, cx| view.queue_session_load(session, cx));
            self.focus_active_tab(window, cx);
            self.set_sidebar_session(session_id, cx);
            cx.notify();
            return;
        }
        let id = self.next_id;
        self.next_id += 1;
        let view =
            Self::create_chat_view(&session.cwd, &self.agent, SessionRenderMode::Main, window, cx);
        let subscription = Self::observe_tab(&view, window, cx);
        view.update(cx, |view, cx| view.queue_session_load(session, cx));
        self.tabs.push(WorkspaceTab::new(
            id,
            WorkspaceTabKind::session(SessionTab::new(id, view, subscription)),
        ));
        self.page = WorkspacePage::sessions();
        self.active = self.tabs.len() - 1;
        self.tab_scroll.scroll_to_item(self.active);
        self.set_sidebar_session(session_id, cx);
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    fn find_session_tab(&self, id: &SessionId, cx: &App) -> Option<usize> {
        self.tabs
            .iter()
            .position(|tab| tab.session().is_some_and(|view| view.read(cx).matches_session(id)))
    }

    pub fn session_management_source(
        &self,
        id: &SessionId,
        cx: &App,
    ) -> Option<&Entity<SessionView>> {
        self.tabs
            .iter()
            .filter_map(WorkspaceTab::session)
            .chain(self.archived_tabs.iter().map(|tab| &tab.view))
            .find(|view| view.read(cx).matches_session(id))
    }

    pub fn forward_session_management_event(
        &mut self,
        event: Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sidebar = self.sidebar.clone();
        window.defer(cx, move |window, cx| {
            sidebar.update(cx, |sidebar, cx| sidebar.apply_event(event, window, cx));
        });
    }

    pub fn fail_session_management(&mut self, id: &SessionId, cx: &mut Context<Self>) {
        let sidebar = self.sidebar.clone();
        let id = id.clone();
        cx.defer(move |cx| {
            sidebar.update(cx, |sidebar, cx| sidebar.fail_session_management(&id, cx));
        });
    }

    pub fn session_management_available(&self, id: &SessionId, cx: &App) -> bool {
        self.tabs
            .iter()
            .filter_map(WorkspaceTab::session)
            .chain(self.archived_tabs.iter().map(|tab| &tab.view))
            .filter(|view| view.read(cx).matches_session(id))
            .all(|view| {
                let view = view.read(cx);
                view.is_session_idle() && !view.execution().is_disconnected()
            })
    }
    pub fn lock_session_management(
        &mut self,
        id: &SessionId,
        locked: bool,
        cx: &mut Context<Self>,
    ) {
        for view in self
            .tabs
            .iter()
            .filter_map(WorkspaceTab::session)
            .chain(self.archived_tabs.iter().map(|tab| &tab.view))
        {
            if view.read(cx).matches_session(id) {
                view.update(cx, |view, cx| view.set_management_lock(locked, cx));
            }
        }
    }
    pub fn sync_session_title(&mut self, id: &SessionId, title: String, cx: &mut Context<Self>) {
        for view in self
            .tabs
            .iter()
            .filter_map(WorkspaceTab::session)
            .chain(self.archived_tabs.iter().map(|tab| &tab.view))
        {
            view.update(cx, |view, cx| view.sync_session_title(id, title.clone(), cx));
        }
        self.sync_sidebar_active_session(cx);
        cx.notify();
    }
    pub fn close_archived_session(&mut self, id: &SessionId, cx: &mut Context<Self>) {
        let active_id = self.tabs.get(self.active).map(|tab| tab.id);
        let mut index = 0;
        while index < self.tabs.len() {
            if self.tabs[index].session().is_some_and(|view| view.read(cx).matches_session(id)) {
                let tab = self.tabs.remove(index);
                if let WorkspaceTabKind::Session(tab) = tab.kind {
                    self.archived_tabs.push(tab);
                }
            } else {
                index += 1;
            }
        }
        self.active = active_id
            .and_then(|id| self.tabs.iter().position(|tab| tab.id == id))
            .unwrap_or_else(|| self.active.min(self.tabs.len().saturating_sub(1)));
        let changed_active = active_id != self.tabs.get(self.active).map(|tab| tab.id);
        if changed_active {
            self.invalidate_file_navigation(cx);
        }
        self.tab_scroll.scroll_to_item(self.active);
        self.sync_sidebar_active_session(cx);
        if changed_active {
            self.defer_active_tab_focus(cx);
        }
        cx.notify();
    }
    pub fn close_deleted_sessions(&mut self, deleted: &HashSet<SessionId>, cx: &mut Context<Self>) {
        self.archived_tabs
            .retain(|tab| !deleted.iter().any(|id| tab.view.read(cx).matches_session(id)));
        let ids: Vec<_> = self
            .tabs
            .iter()
            .filter(|tab| {
                tab.session()
                    .is_some_and(|view| deleted.iter().any(|id| view.read(cx).matches_session(id)))
            })
            .map(|tab| tab.id)
            .collect();
        for id in ids {
            self.close_tab(id, cx);
        }
    }

    pub fn close_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        if self
            .file_view(id)
            .is_some_and(|view| view.read(cx).is_dirty() || view.read(cx).is_saving())
            || self.pending_file_operations.iter().any(|operation| operation.tab_id() == Some(id))
        {
            return;
        }
        self.remove_tab(id, cx);
    }

    fn remove_tab(&mut self, id: usize, cx: &mut Context<Self>) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        if self.tabs[index].is_pending(cx)
            || self.tabs[index].file().is_some_and(|view| view.read(cx).is_saving())
        {
            return;
        }
        let active_id = self.tabs.get(self.active).map(|tab| tab.id);
        self.tabs.remove(index);
        self.active = active_id
            .and_then(|id| self.tabs.iter().position(|tab| tab.id == id))
            .unwrap_or_else(|| index.min(self.tabs.len().saturating_sub(1)));
        let changed_active = active_id != self.tabs.get(self.active).map(|tab| tab.id);
        if changed_active {
            self.invalidate_file_navigation(cx);
        }
        self.tab_scroll.scroll_to_item(self.active);
        self.sync_sidebar_active_session(cx);
        if changed_active {
            self.defer_active_tab_focus(cx);
        }
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
        let operations = self
            .tabs
            .iter()
            .filter(|tab| {
                !tab.is_pending(cx)
                    && match command {
                        TabCommand::Close(id) => tab.id == id,
                        TabCommand::CloseAll => true,
                        TabCommand::CloseOthers(id) => tab.id != id,
                    }
            })
            .map(|tab| PendingFileOperation::close(tab.id))
            .collect();
        self.request_file_operations(operations, window, cx);
    }

    fn defer_active_tab_focus(&self, cx: &mut Context<Self>) {
        let id = self.tabs.get(self.active).map(|tab| tab.id);
        let revision = self.file_navigation_revision;
        let window = self.window;
        let owner = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = window.update(cx, |_, window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    let already_focused = this.active_file_view().is_some_and(|view| {
                        view.read(cx).focus_handle(cx).contains_focused(window, cx)
                    }) || this.active_view().is_some_and(|view| {
                        view.read(cx).input().focus_handle(cx).contains_focused(window, cx)
                    });
                    if this.file_navigation_revision == revision
                        && this.tabs.get(this.active).map(|tab| tab.id) == id
                        && !window.has_active_dialog(cx)
                        && !already_focused
                    {
                        this.focus_active_tab(window, cx);
                    }
                });
            });
        });
    }

    fn focus_active_tab(&self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_sessions_visible() || window.has_active_dialog(cx) {
            return;
        }
        self.focus_handle.focus(window, cx);
        let id = self.tabs.get(self.active).map(|tab| tab.id);
        let revision = self.file_navigation_revision;
        let owner = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = owner.update(cx, |this, cx| {
                if !this.is_sessions_visible()
                    || this.file_navigation_revision != revision
                    || this.tabs.get(this.active).map(|tab| tab.id) != id
                    || !this.focus_handle.is_focused(window)
                {
                    return;
                }
                if let Some(view) = this.active_file_view() {
                    view.update(cx, |view, cx| view.focus(window, cx));
                } else if let Some(view) = this.active_view() {
                    let input = view.read(cx).input().clone();
                    input.update(cx, |input, cx| input.focus(window, cx));
                }
            });
        });
    }

    pub fn select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if index >= self.tabs.len()
            || self.file_confirmation.is_some()
            || !self.pending_file_operations.is_empty()
            || window.has_active_dialog(cx)
        {
            return;
        }
        if !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx)) {
            return;
        }
        if !self.is_sessions_visible() {
            self.show_sessions(window, cx);
        }
        self.file_navigation_revision = self.file_navigation_revision.wrapping_add(1);
        self.active = index;
        self.tab_scroll.scroll_to_item(index);
        self.sync_sidebar_active_session(cx);
        self.focus_active_tab(window, cx);
        cx.notify();
    }

    pub fn switch_tab(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_sessions_visible() {
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
    fn render_tab_bar(
        &self,
        p: Palette,
        sidebar_available: Option<Pixels>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tabs: Vec<_> = if !self.is_sessions_visible() {
            Vec::new()
        } else {
            self.tabs
                .iter()
                .enumerate()
                .map(|(index, tab)| self.render_tab(index, tab, p, cx))
                .collect()
        };
        let sidebar_visible = self.sidebar_visible && sidebar_available.is_some();
        let label = if sidebar_visible { "收起侧栏" } else { "展开侧栏" };
        let icon = if sidebar_visible { IconName::PanelLeftClose } else { IconName::PanelLeftOpen };
        let shortcut = if cfg!(target_os = "macos") { "⌘B" } else { "Ctrl+B" };
        div()
            .id("tab-bar")
            .h_9()
            .flex_shrink_0()
            .px_2()
            .gap_1()
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .child(
                Button::new("toggle-sidebar")
                    .track_focus(&self.sidebar_toggle_focus_handle)
                    .ghost()
                    .small()
                    .icon(Icon::new(icon))
                    .accessibility_label(label)
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .disabled(sidebar_available.is_none())
                    .tooltip(if sidebar_available.is_some() {
                        format!("{label} {shortcut}")
                    } else {
                        "窗口较窄，加宽窗口或收起详情后可展开侧栏".into()
                    })
                    .on_click(|_, window, cx| {
                        cx.stop_propagation();
                        window.dispatch_action(Box::new(ToggleSidebar), cx);
                    }),
            )
            .child(
                div()
                    .id("tab-scroll")
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .items_center()
                    .gap_1p5()
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
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(|_, _, cx| cx.stop_propagation())
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
            view.read(cx).tab_menu(menu, id, owner.clone(), cx)
        })
        .into_any_element()
    }

    fn tab_menu(
        &self,
        mut menu: PopupMenu,
        id: usize,
        owner: WeakEntity<Self>,
        cx: &App,
    ) -> PopupMenu {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return menu;
        };
        if let Some(view) = tab.file() {
            let ready = view.read(cx).is_ready();
            let open = view.read(cx).is_open();
            let loading = view.read(cx).is_loading();
            let search = if cfg!(target_os = "macos") {
                "搜索文件内容…（⌘F）"
            } else {
                "搜索文件内容…（Ctrl+F）"
            };
            let copy = if cfg!(target_os = "macos") {
                "复制全文（⌘⇧C）"
            } else {
                "复制全文（Ctrl+Shift+C）"
            };
            for (label, command, disabled) in [
                (search, FileTabCommand::search(), !ready),
                (copy, FileTabCommand::copy_all(), !ready),
                ("重新读取文件…", FileTabCommand::reload(), !open || loading),
                ("复制绝对路径", FileTabCommand::copy_absolute_path(), !open),
                ("复制相对路径", FileTabCommand::copy_relative_path(), !open),
                ("在文件树中定位", FileTabCommand::locate(), !open),
            ] {
                let owner = owner.clone();
                menu = menu.item(PopupMenuItem::new(label).disabled(disabled).on_click(
                    move |_, window, cx| {
                        let owner = owner.clone();
                        window.defer(cx, move |window, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                this.execute_file_tab_command(id, command, window, cx);
                            });
                        });
                    },
                ));
            }
            menu = menu.separator();
        }
        let has_others = self.tabs.iter().any(|tab| tab.id != id && !tab.is_pending(cx));
        let has_closable = self.tabs.iter().any(|tab| !tab.is_pending(cx));
        for (label, command, disabled) in [
            ("关闭", TabCommand::close(id), tab.is_pending(cx)),
            ("关闭其他标签", TabCommand::close_others(id), !has_others),
            ("关闭全部标签", TabCommand::close_all(), !has_closable),
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

    fn execute_file_tab_command(
        &mut self,
        id: usize,
        command: FileTabCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let Some(view) = self.tabs[index].file().cloned() else {
            return;
        };
        let Some(target) = view.read(cx).target().cloned() else {
            return;
        };
        match command {
            FileTabCommand::Search => {
                if !view.read(cx).is_ready() {
                    return;
                }
                self.select_tab(index, window, cx);
                if self.tabs.get(self.active).is_none_or(|tab| tab.id != id) {
                    return;
                }
                let revision = self.file_navigation_revision;
                let owner = cx.weak_entity();
                window.on_next_frame(move |window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.is_sessions_visible()
                            && this.file_navigation_revision == revision
                            && this.tabs.get(this.active).is_some_and(|tab| tab.id == id)
                            && let Some(view) = this
                                .tabs
                                .iter()
                                .find(|tab| tab.id == id)
                                .and_then(WorkspaceTab::file)
                            && (this.focus_handle.is_focused(window)
                                || view.read(cx).focus_handle(cx).contains_focused(window, cx))
                        {
                            view.update(cx, |view, cx| view.search(window, cx));
                        }
                    });
                });
            },
            FileTabCommand::CopyAll => view.update(cx, |view, cx| view.copy_all(cx)),
            FileTabCommand::Reload => {
                if !view.read(cx).is_loading() {
                    self.request_file_operations(
                        VecDeque::from([PendingFileOperation::reload(id)]),
                        window,
                        cx,
                    );
                }
            },
            FileTabCommand::CopyAbsolutePath => {
                cx.write_to_clipboard(ClipboardItem::new_string(
                    target.path().to_string_lossy().into_owned(),
                ));
            },
            FileTabCommand::CopyRelativePath => {
                let path = self.normalize_file_path(target.path(), cx);
                let directory = self.normalize_file_path(target.directory(), cx);
                let relative = path.strip_prefix(directory).unwrap_or(&path);
                cx.write_to_clipboard(ClipboardItem::new_string(
                    relative.to_string_lossy().into_owned(),
                ));
            },
            FileTabCommand::Locate => {
                self.select_tab(index, window, cx);
                if self.tabs.get(self.active).is_none_or(|tab| tab.id != id) {
                    return;
                }
                self.file_tree.update(cx, |tree, cx| tree.reveal(target.path(), cx));
                self.select_sidebar_page(SidebarPage::files(), window, cx);
            },
        }
    }

    fn file_tab_title(&self, id: usize, target: &OpenFile, cx: &App) -> String {
        let name = target.path().file_name().unwrap_or(target.path().as_os_str());
        let duplicate = self.tabs.iter().any(|tab| {
            tab.id != id
                && tab
                    .file()
                    .and_then(|view| view.read(cx).target())
                    .is_some_and(|target| target.path().file_name() == Some(name))
        });
        if duplicate {
            let parent = target.path().parent().unwrap_or(target.path());
            let directory = self.normalize_file_path(target.directory(), cx);
            let parent = self.normalize_file_path(parent, cx);
            let relative = parent.strip_prefix(&directory).unwrap_or(&parent);
            let relative = relative.to_owned();
            let same_parent = self.tabs.iter().any(|tab| {
                tab.id != id
                    && tab.file().and_then(|view| view.read(cx).target()).is_some_and(|other| {
                        other.path().file_name() == Some(name)
                            && other.path().parent().is_some_and(|parent| {
                                let parent = self.normalize_file_path(parent, cx);
                                let root = self.normalize_file_path(other.directory(), cx);
                                parent.strip_prefix(root).unwrap_or(&parent) == relative
                            })
                    })
            });
            let parent = if same_parent {
                parent.display().to_string()
            } else if relative.as_os_str().is_empty() {
                ".".into()
            } else {
                relative.display().to_string()
            };
            format!("{} — {parent}", name.to_string_lossy())
        } else {
            name.to_string_lossy().into_owned()
        }
    }

    fn render_empty_main(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let project = self.cwd.file_name().unwrap_or(self.cwd.as_os_str()).to_string_lossy();
        div()
            .size_full()
            .min_h_0()
            .p_8()
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
                    .child(Icon::new(IconName::MessageCircle).size_12().text_color(p.muted()))
                    .child(
                        div()
                            .mt_2()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(p.foreground())
                            .child(project.into_owned()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .line_height(rems(1.25))
                            .text_color(p.muted())
                            .child("从侧栏选择会话，或在当前项目开始新的会话。"),
                    )
                    .child(
                        Button::new("empty-new-session")
                            .small()
                            .mt_4()
                            .icon(IconName::Plus)
                            .label("新建会话")
                            .disabled(!self.sidebar.read(cx).can_start_new_chat())
                            .on_click(|_, window, cx| {
                                window.dispatch_action(Box::new(NewSession), cx)
                            }),
                    )
                    .when(cfg!(target_os = "macos"), |empty| {
                        empty.child(
                            div()
                                .mt_2()
                                .text_xs()
                                .text_color(p.muted())
                                .child("⌘N 新建会话 · ⌘B 切换侧栏"),
                        )
                    }),
            )
            .into_any_element()
    }

    fn render_tab(
        &self,
        index: usize,
        tab: &WorkspaceTab,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = tab.id;
        let active = index == self.active && self.is_sessions_visible();
        let pending = tab.is_pending(cx);
        let (title, tooltip, status, indicator) = match &tab.kind {
            WorkspaceTabKind::Session(tab) => {
                let view = tab.view.read(cx);
                let title = view.tab_title();
                let state = view.execution_state();
                (title.clone(), title, state.label(), state.render_tab_indicator(p))
            },
            WorkspaceTabKind::File(tab) => {
                let view = tab.view.read(cx);
                let title = view
                    .target()
                    .map(|target| self.file_tab_title(id, target, cx))
                    .unwrap_or_else(|| "文件".into());
                let tooltip = view
                    .target()
                    .map(|target| target.path().display().to_string())
                    .unwrap_or_else(|| title.clone());
                let loading = view.is_loading();
                let failed = view.has_error() || view.save_error().is_some();
                let indicator = div()
                    .size_4()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(if loading {
                        Spinner::new().xsmall().into_any_element()
                    } else {
                        Icon::new(if failed { IconName::Info } else { IconName::FileCode })
                            .size_3()
                            .text_color(if failed { cx.theme().danger } else { p.muted() })
                            .into_any_element()
                    })
                    .into_any_element();
                let status = if loading {
                    "读取中"
                } else if view.save_error().is_some() {
                    "自动保存失败"
                } else if failed {
                    "读取失败"
                } else {
                    "文件"
                };
                (title, tooltip, status, indicator)
            },
        };
        let tab_label = format!("切换到标签 {title} · {status}");
        let drag_title = title.clone().into();
        let tab = div()
            .id(("tab", id))
            .h_7()
            .flex_shrink_0()
            .w(rems(11.25))
            .min_w(rems(11.25))
            .pr_0p5()
            .gap_1()
            .rounded(cx.theme().radius_tokens().md)
            .flex()
            .items_center()
            .bg(if active { p.background() } else { p.background().opacity(0.) })
            .when(!active, |tab| tab.hover(|style| style.bg(p.hover())))
            .child(
                Button::new(("tab-button", id))
                    .custom(ButtonCustomVariant::new(cx).active(cx.theme().button_active))
                    .small()
                    .h_full()
                    .px_0()
                    .rounded(cx.theme().radius_tokens().md)
                    .flex_1()
                    .min_w_0()
                    .accessibility_label(tab_label)
                    .tooltip(format!("{tooltip} · {status}"))
                    .child(indicator)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if let Some(index) = this.tabs.iter().position(|tab| tab.id == id) {
                            this.select_tab(index, window, cx);
                        }
                    }))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .truncate()
                            .line_clamp(1)
                            .text_left()
                            .text_sm()
                            .text_color(if active { p.foreground() } else { p.muted() })
                            .child(title),
                    )
                    .when(
                        tab.file().is_some_and(|view| view.read(cx).save_error().is_some()),
                        |button| button.child(div().text_xs().text_color(p.muted()).child(status)),
                    ),
            )
            .child(
                Button::new(("close-tab", id))
                    .icon(Icon::new(IconName::X).size(rems(0.6875)))
                    .ghost()
                    .xsmall()
                    .flex_shrink_0()
                    .text_color(p.muted())
                    .disabled(pending)
                    .tooltip_with_action("关闭标签", &CloseTab, None)
                    .accessibility_label("关闭标签")
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.execute_tab_command(TabCommand::close(id), window, cx);
                    })),
            );
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

impl WorkspaceView {
    pub fn preferences<'a>(&'a self, cx: &'a App) -> &'a Preferences {
        self.settings.read(cx).preferences()
    }

    pub fn settings_state(&self) -> &Entity<SettingsState> {
        &self.settings
    }

    pub fn is_sessions_visible(&self) -> bool {
        self.page == WorkspacePage::sessions()
    }

    pub fn is_usage_visible(&self) -> bool {
        self.page == WorkspacePage::usage()
    }

    pub fn is_extensions_visible(&self) -> bool {
        self.page == WorkspacePage::extensions()
    }

    pub fn open_usage(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.page = WorkspacePage::usage();
        self.invalidate_file_navigation(cx);
        self.sync_sidebar_active_session(cx);
        self.usage.update(cx, |view, cx| view.open(window, cx));
        self.sidebar.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn open_extensions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let directory = self.current_directory(cx);
        self.page = WorkspacePage::extensions();
        self.invalidate_file_navigation(cx);
        self.sync_sidebar_active_session(cx);
        self.extensions.update(cx, |view, cx| view.open(directory, window, cx));
        self.sidebar.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn show_sessions(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_sessions_visible() {
            return;
        }
        self.page = WorkspacePage::sessions();
        self.invalidate_file_navigation(cx);
        self.sync_sidebar_active_session(cx);
        self.focus_active_tab(window, cx);
        self.sidebar.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn open_settings(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.file_confirmation.is_some()
            || !self.pending_file_operations.is_empty()
            || window.has_active_dialog(cx)
        {
            return;
        }
        self.settings.update(cx, |settings, cx| settings.open_settings(window, cx));
    }

    pub fn notify_settings_sidebar(&self, cx: &mut Context<Self>) {
        self.sidebar.update(cx, |_, cx| cx.notify());
    }

    pub fn notify_settings_tabs(&self, cx: &mut Context<Self>) {
        for tab in &self.tabs {
            match &tab.kind {
                WorkspaceTabKind::Session(tab) => tab.view.update(cx, |_, cx| cx.notify()),
                WorkspaceTabKind::File(tab) => tab.view.update(cx, |_, cx| cx.notify()),
            }
        }
    }

    pub fn remeasure_timelines(&self, cx: &mut Context<Self>) {
        self.sidebar.read(cx).remeasure_timeline();
        for view in self.tabs.iter().filter_map(WorkspaceTab::session) {
            view.read(cx).remeasure_timeline();
        }
        cx.notify();
    }
}
