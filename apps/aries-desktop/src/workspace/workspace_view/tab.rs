use std::path::PathBuf;

use gpui_kit::assets::IconName;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::files::{FileView, OpenFile};
use crate::workspace::git::diff_view::GitDiffView;
use crate::workspace::settings::palette::Palette;
use crate::workspace::workspace_view::{WorkspacePage, WorkspaceView};

pub struct Tab<T> {
    id: usize,
    view: Entity<T>,
    _subscription: Subscription,
}

impl<T> Tab<T> {
    pub fn new(id: usize, view: Entity<T>, subscription: Subscription) -> Self {
        Self { id, view, _subscription: subscription }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn view(&self) -> &Entity<T> {
        &self.view
    }
}

pub struct FileTab {
    tab: Tab<FileView>,
    _close_subscription: Subscription,
    canonical_path: Option<PathBuf>,
    identity_task: Option<Task<()>>,
    pending_target: Option<OpenFile>,
}

impl FileTab {
    pub fn new(tab: Tab<FileView>, close_subscription: Subscription) -> Self {
        Self {
            tab,
            _close_subscription: close_subscription,
            canonical_path: None,
            identity_task: None,
            pending_target: None,
        }
    }

    pub fn id(&self) -> usize {
        self.tab.id()
    }

    pub fn view(&self) -> &Entity<FileView> {
        self.tab.view()
    }

    pub fn canonical_path(&self) -> Option<&PathBuf> {
        self.canonical_path.as_ref()
    }

    pub fn pending_target(&self) -> Option<&OpenFile> {
        self.pending_target.as_ref()
    }

    pub fn set_pending_target(&mut self, target: OpenFile) {
        self.pending_target = Some(target);
    }

    pub fn take_pending_target(&mut self) -> Option<OpenFile> {
        self.pending_target.take()
    }

    pub fn reset_identity(&mut self) {
        self.identity_task = None;
        self.canonical_path = None;
    }

    pub fn start_identity(&mut self, task: Task<()>) {
        self.identity_task = Some(task);
    }

    pub fn settle_identity(&mut self, canonical: Option<PathBuf>) {
        self.identity_task = None;
        self.canonical_path = canonical;
    }

    fn meta(&self, owner: &WorkspaceView, p: Palette, cx: &App) -> TabMeta {
        let view = self.view().read(cx);
        let target = view.target();
        let title = target
            .map(|target| owner.file_tab_title(self.id(), target, cx))
            .unwrap_or_else(|| "文件".into());
        let tooltip = target
            .map(|target| target.path().display().to_string())
            .unwrap_or_else(|| title.clone());
        let loading = view.is_loading();
        let failed = view.has_error() || view.save_error().is_some();
        let indicator = if loading {
            Spinner::new().xsmall().into_any_element()
        } else {
            Icon::new(if failed { IconName::Info } else { IconName::FileCode })
                .size_3()
                .text_color(if failed { cx.theme().danger } else { p.muted() })
                .into_any_element()
        };
        let status = if loading {
            "读取中"
        } else if view.save_error().is_some() {
            "自动保存失败"
        } else if failed {
            "读取失败"
        } else {
            "文件"
        };
        let meta = TabMeta::new(title, tooltip, status, tab_indicator(indicator));
        if view.save_error().is_some() { meta.notice(status) } else { meta }
    }
}

pub struct DiffTab {
    tab: Tab<GitDiffView>,
    session: Entity<SessionView>,
}

impl DiffTab {
    pub fn new(tab: Tab<GitDiffView>, session: Entity<SessionView>) -> Self {
        Self { tab, session }
    }

    pub fn id(&self) -> usize {
        self.tab.id()
    }

    pub fn view(&self) -> &Entity<GitDiffView> {
        self.tab.view()
    }

    pub fn session(&self) -> &Entity<SessionView> {
        &self.session
    }

    fn meta(&self, p: Palette, cx: &App) -> TabMeta {
        let view = self.view().read(cx);
        let target = view.target();
        let title = target
            .and_then(|target| target.path().file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "变更".into());
        let tooltip = target
            .map(|target| format!("{} · {}", target.group().title(), target.path().display()))
            .unwrap_or_else(|| title.clone());
        let status = target.map(|target| target.group().title()).unwrap_or("变更");
        let indicator = tab_indicator(
            Icon::new(IconName::FileDiff).size_3().text_color(p.muted()).into_any_element(),
        );
        TabMeta::new(title, tooltip, status, indicator)
    }
}

pub struct PageTab {
    id: usize,
    page: WorkspacePage,
}

impl PageTab {
    pub fn new(id: usize, page: WorkspacePage) -> Self {
        Self { id, page }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn page(&self) -> WorkspacePage {
        self.page
    }

    fn meta(&self, p: Palette) -> TabMeta {
        let page = self.page();
        let title = page.title();
        let indicator =
            tab_indicator(Icon::new(page.icon()).size_3().text_color(p.muted()).into_any_element());
        TabMeta::new(title, title, page.status(), indicator)
    }
}

pub struct TabMeta {
    pub title: SharedString,
    pub tooltip: SharedString,
    pub status: SharedString,
    pub indicator: AnyElement,
    pub notice: Option<SharedString>,
}

impl TabMeta {
    pub fn new(
        title: impl Into<SharedString>,
        tooltip: impl Into<SharedString>,
        status: impl Into<SharedString>,
        indicator: AnyElement,
    ) -> Self {
        Self {
            title: title.into(),
            tooltip: tooltip.into(),
            status: status.into(),
            indicator,
            notice: None,
        }
    }

    pub fn notice(self, notice: impl Into<SharedString>) -> Self {
        Self { notice: Some(notice.into()), ..self }
    }
}

pub enum WorkspaceTab {
    Session(Tab<SessionView>),
    File(FileTab),
    Diff(DiffTab),
    Page(PageTab),
}

impl WorkspaceTab {
    pub fn new_session(id: usize, view: Entity<SessionView>, subscription: Subscription) -> Self {
        Self::Session(Tab::new(id, view, subscription))
    }

    pub fn resumed_session(tab: Tab<SessionView>) -> Self {
        Self::Session(tab)
    }

    pub fn new_file(tab: FileTab) -> Self {
        Self::File(tab)
    }

    pub fn new_diff(tab: DiffTab) -> Self {
        Self::Diff(tab)
    }

    pub fn new_page(id: usize, page: WorkspacePage) -> Self {
        Self::Page(PageTab::new(id, page))
    }

    pub fn id(&self) -> usize {
        match self {
            Self::Session(tab) => tab.id(),
            Self::File(tab) => tab.id(),
            Self::Diff(tab) => tab.id(),
            Self::Page(tab) => tab.id(),
        }
    }

    pub fn session(&self) -> Option<&Entity<SessionView>> {
        match self {
            Self::Session(tab) => Some(tab.view()),
            _ => None,
        }
    }

    pub fn file(&self) -> Option<&Entity<FileView>> {
        match self {
            Self::File(tab) => Some(tab.view()),
            _ => None,
        }
    }

    pub fn diff(&self) -> Option<&Entity<GitDiffView>> {
        match self {
            Self::Diff(tab) => Some(tab.view()),
            _ => None,
        }
    }

    pub fn diff_session(&self) -> Option<&Entity<SessionView>> {
        match self {
            Self::Diff(tab) => Some(tab.session()),
            _ => None,
        }
    }

    pub fn page(&self) -> Option<WorkspacePage> {
        match self {
            Self::Page(tab) => Some(tab.page()),
            _ => None,
        }
    }

    pub fn is_pending(&self, cx: &App) -> bool {
        self.session().is_some_and(|view| view.read(cx).management().is_pending())
    }

    pub fn is_saving(&self, cx: &App) -> bool {
        self.file().is_some_and(|view| view.read(cx).is_saving())
    }

    pub fn meta(&self, owner: &WorkspaceView, p: Palette, cx: &App) -> TabMeta {
        match self {
            Self::Session(tab) => {
                let view = tab.view().read(cx);
                let title = view.tab_title();
                let state = view.execution_state();
                TabMeta::new(title.clone(), title, state.label(), state.render_tab_indicator(p))
            },
            Self::File(tab) => tab.meta(owner, p, cx),
            Self::Diff(tab) => tab.meta(p, cx),
            Self::Page(tab) => tab.meta(p),
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        match self {
            Self::Session(tab) => {
                let input = tab.view().read(cx).input().clone();
                input.update(cx, |input, cx| input.focus(window, cx));
            },
            Self::File(tab) => tab.view().update(cx, |view, cx| view.focus(window, cx)),
            Self::Diff(tab) => tab.view().update(cx, |view, cx| view.focus(window, cx)),
            Self::Page(_) => {},
        }
    }

    pub fn contains_focused(&self, window: &Window, cx: &App) -> bool {
        match self {
            Self::Session(tab) => {
                let input = tab.view().read(cx).input();
                input.focus_handle(cx).contains_focused(window, cx)
            },
            Self::File(tab) => tab.view().read(cx).focus_handle(cx).contains_focused(window, cx),
            Self::Diff(tab) => tab.view().read(cx).is_focused(window, cx),
            Self::Page(_) => false,
        }
    }

    pub fn notify(&self, cx: &mut App) {
        match self {
            Self::Session(tab) => tab.view().update(cx, |_, cx| cx.notify()),
            Self::File(tab) => tab.view().update(cx, |_, cx| cx.notify()),
            Self::Diff(tab) => tab.view().update(cx, |_, cx| cx.notify()),
            Self::Page(_) => {},
        }
    }
}

fn tab_indicator(child: AnyElement) -> AnyElement {
    div()
        .size_4()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .child(child)
        .into_any_element()
}
