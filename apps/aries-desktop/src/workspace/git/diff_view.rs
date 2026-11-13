use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonGroup, ButtonVariants};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::diff::{
    Diff, DiffDecoration, DiffFile, DiffFileStatus, DiffLinePosition, DiffLineRange, DiffMode,
    DiffState,
};
use gpui_kit::component::input::InputState;
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::error::details;
use crate::workspace::SessionView;
use crate::workspace::git::diff_operations::DiscardBackupEntry;
use crate::workspace::git::diff_patch::{DiffPatch, PatchScope};
use crate::workspace::git::error::DiffLoadError;
use crate::workspace::git::repository::{ChangeGroup, GitInfo, read_file_diff};

mod tools;

#[derive(Clone, Debug, PartialEq, Eq)]
struct DiffTarget {
    path: PathBuf,
    group: ChangeGroup,
}

impl DiffTarget {
    fn new(path: PathBuf, group: ChangeGroup) -> Self {
        Self { path, group }
    }
}

pub struct StageNavigation {
    target: DiffTarget,
    revision: u64,
    index: usize,
}

impl StageNavigation {
    fn new(target: DiffTarget, revision: u64, index: usize) -> Self {
        Self { target, revision, index }
    }
}

pub struct DiffTargetChanged;

impl DiffTargetChanged {
    fn new() -> Self {
        Self
    }
}

pub struct GitDiffView {
    owner: WeakEntity<SessionView>,
    directory: PathBuf,
    target: Option<DiffTarget>,
    targets: Vec<DiffTarget>,
    state: Entity<DiffState>,
    patch: Arc<String>,
    error: Option<String>,
    model: Option<Arc<DiffPatch>>,
    tool_error: Option<String>,
    selected_scope: Option<PatchScope>,
    selection_snapshot: Option<DiffLineRange>,
    selected_count: usize,
    current_hunk: Option<usize>,
    location: Option<DiffLinePosition>,
    search_input: Option<Entity<InputState>>,
    search_subscription: Option<Subscription>,
    search_open: bool,
    search_revision: u64,
    search_task: Option<Task<()>>,
    search_loading: bool,
    matches: Vec<DiffLinePosition>,
    match_index: Option<usize>,
    decorations: Vec<DiffDecoration>,
    backups: Vec<DiscardBackupEntry>,
    backup_task: Option<Task<()>>,
    loading: bool,
    pending: bool,
    revision: u64,
    task: Option<Task<()>>,
    previous_focus: Option<FocusHandle>,
    _subscription: Subscription,
}

impl EventEmitter<DiffTargetChanged> for GitDiffView {}

impl GitDiffView {
    pub fn new(owner: WeakEntity<SessionView>, directory: PathBuf, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| DiffState::new(Vec::new(), cx));
        let subscription = cx.observe(&state, |this, _, cx| {
            this.selection_changed(cx);
            cx.notify();
        });
        Self {
            owner,
            directory,
            target: None,
            targets: Vec::new(),
            state,
            patch: Arc::new(String::new()),
            error: None,
            model: None,
            tool_error: None,
            selected_scope: None,
            selection_snapshot: None,
            selected_count: 0,
            current_hunk: None,
            location: None,
            search_input: None,
            search_subscription: None,
            search_open: false,
            search_revision: 0,
            search_task: None,
            search_loading: false,
            matches: Vec::new(),
            match_index: None,
            decorations: Vec::new(),
            backups: Vec::new(),
            backup_task: None,
            loading: false,
            pending: false,
            revision: 0,
            task: None,
            previous_focus: None,
            _subscription: subscription,
        }
    }

    pub fn is_open(&self) -> bool {
        self.target.is_some()
    }

    pub fn is_selected(&self, path: &Path, group: ChangeGroup) -> bool {
        self.target.as_ref().is_some_and(|target| target.path == path && target.group == group)
    }

    pub fn set_directory(&mut self, directory: &Path, cx: &mut Context<Self>) {
        if self.directory == directory {
            return;
        }
        self.directory = directory.to_owned();
        let was_open = self.is_open();
        self.target = None;
        self.targets.clear();
        self.previous_focus = None;
        self.reset(cx);
        if was_open {
            cx.emit(DiffTargetChanged::new());
        }
    }

    pub fn open(
        &mut self,
        path: PathBuf,
        group: ChangeGroup,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_open() {
            self.previous_focus = window.focused(cx);
        }
        if !self.is_selected(&path, group) {
            self.target = Some(DiffTarget::new(path, group));
            self.reset(cx);
            self.load_backups(window, cx);
            cx.emit(DiffTargetChanged::new());
        }
        self.initialize_search(window, cx);
        self.refresh(cx);
        self.state.focus_handle(cx).focus(window, cx);
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        self.revision += 1;
        self.task = None;
        self.loading = false;
        self.pending = false;
        self.patch = Arc::new(String::new());
        self.error = None;
        self.reset_tools();
        self.state.update(cx, |state, cx| state.set_files(Vec::new(), cx));
        cx.notify();
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.target = None;
        self.reset(cx);
        cx.emit(DiffTargetChanged::new());
        if let Some(focus) = self.previous_focus.take() {
            window.focus(&focus, cx);
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.target.clone() else {
            return;
        };
        if self.loading {
            self.pending = true;
            return;
        }
        self.loading = true;
        self.pending = false;
        let directory = self.directory.clone();
        let previous_patch = self.patch.clone();
        let revision = self.revision;
        self.task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    let patch = read_file_diff(&directory, &target.path, target.group)?;
                    if patch == *previous_patch {
                        return Ok::<_, DiffLoadError>(None);
                    }
                    let files = DiffFile::parse(&patch)
                        .map_err(|error| DiffLoadError::display_patch(&target.path, error))?;
                    let model = if patch.is_empty() {
                        Ok(None)
                    } else {
                        DiffPatch::parse(&patch).map(|model| Some(Arc::new(model)))
                    };
                    Ok(Some((Arc::new(patch), files, model)))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.revision != revision {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(result) => {
                        if let Some((patch, files, model)) = result {
                            this.state.update(cx, |state, cx| state.set_files(files, cx));
                            this.patch = patch;
                            match model {
                                Ok(model) => {
                                    this.current_hunk = model
                                        .as_ref()
                                        .filter(|model| !model.hunks().is_empty())
                                        .map(|_| 0);
                                    this.model = model;
                                    this.tool_error = None;
                                },
                                Err(error) => {
                                    this.model = None;
                                    this.current_hunk = None;
                                    this.tool_error =
                                        Some(format!("部分操作不可用：{}", details(&error)));
                                },
                            }
                            this.location = None;
                            this.selection_snapshot = None;
                            this.selected_scope = None;
                            this.selected_count = 0;
                            this.selection_changed(cx);
                            this.update_search(cx);
                        }
                        this.error = None;
                    },
                    Err(error) => this.error = Some(details(&error)),
                }
                if this.pending {
                    this.refresh(cx);
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub fn set_targets(&mut self, info: Option<&GitInfo>, cx: &mut Context<Self>) {
        let files = info.and_then(GitInfo::changed_files).unwrap_or_default();
        let targets = [ChangeGroup::Staged, ChangeGroup::Unstaged]
            .into_iter()
            .flat_map(|group| {
                files.iter().filter_map(move |file| {
                    file.change(group)
                        .map(|change| DiffTarget::new(change.path().to_owned(), group))
                })
            })
            .collect::<Vec<_>>();
        if self.targets != targets {
            self.targets = targets;
            cx.notify();
        }
    }

    pub fn stage_navigation(&self, path: &Path) -> Option<StageNavigation> {
        let target = self.target.as_ref()?;
        if target.group != ChangeGroup::Unstaged || target.path != path {
            return None;
        }
        let index = self.group_targets(target.group).position(|candidate| candidate == target)?;
        Some(StageNavigation::new(target.clone(), self.revision, index))
    }

    pub fn advance_after_stage(
        &mut self,
        navigation: StageNavigation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.revision != navigation.revision
            || self.target.as_ref() != Some(&navigation.target)
            || self.targets.contains(&navigation.target)
        {
            return;
        }
        let next = self
            .group_targets(ChangeGroup::Unstaged)
            .nth(navigation.index)
            .or_else(|| self.group_targets(ChangeGroup::Unstaged).last())
            .cloned();
        if let Some(target) = next {
            self.open(target.path, target.group, window, cx);
        }
    }

    fn group_targets(&self, group: ChangeGroup) -> impl Iterator<Item = &DiffTarget> {
        self.targets.iter().filter(move |target| target.group == group)
    }

    fn adjacent_target(&self, offset: isize) -> Option<&DiffTarget> {
        let target = self.target.as_ref()?;
        let ix = self.group_targets(target.group).position(|candidate| candidate == target);
        let ix = match ix {
            Some(ix) => ix.checked_add_signed(offset)?,
            None if offset > 0 => 0,
            None => return None,
        };
        self.group_targets(target.group).nth(ix)
    }

    fn set_group(&mut self, group: ChangeGroup, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.target.as_ref() else {
            return;
        };
        if current.group == group {
            return;
        }
        let target = self
            .group_targets(group)
            .find(|target| target.path == current.path)
            .or_else(|| self.group_targets(group).next())
            .cloned();
        if let Some(target) = target {
            self.switch_file(target, window, cx);
        }
    }

    fn render_groups(&self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.controls_busy(cx);
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .px_3()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.5))
            .child(ButtonGroup::new("diff-groups").small().outline().children(
                [ChangeGroup::Unstaged, ChangeGroup::Staged].map(|group| {
                    let count = self.group_targets(group).count();
                    let id = match group {
                        ChangeGroup::Unstaged => "unstaged-diff",
                        ChangeGroup::Staged => "staged-diff",
                    };
                    Button::new(id)
                        .label(format!("{} {count}", group.title()))
                        .selected(self.target.as_ref().is_some_and(|target| target.group == group))
                        .disabled(busy || count == 0)
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.set_group(group, window, cx);
                        }))
                }),
            ))
            .into_any_element()
    }

    fn switch_file(&mut self, target: DiffTarget, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open() || self.controls_busy(cx) || !self.targets.contains(&target) {
            return;
        }
        self.open(target.path, target.group, window, cx);
    }

    fn move_file(&mut self, offset: isize, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(target) = self.adjacent_target(offset).cloned() {
            self.switch_file(target, window, cx);
        }
    }

    fn render_file_selector(&self, path: &str, cx: &mut Context<Self>) -> AnyElement {
        let view = cx.weak_entity();
        let directory = self.directory.clone();
        let group = self.target.as_ref().map(|target| target.group);
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                Button::new("select-diff-file")
                    .small()
                    .ghost()
                    .w_full()
                    .min_w_0()
                    .disabled(
                        self.controls_busy(cx)
                            || group.is_none_or(|group| self.group_targets(group).next().is_none()),
                    )
                    .accessibility_label(format!("切换 diff 文件：{path}"))
                    .tooltip(path.to_owned())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .w_full()
                            .min_w_0()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_left()
                                    .child(path.to_owned()),
                            )
                            .child(Icon::new(IconName::ChevronDown).size_3().flex_shrink_0()),
                    )
                    .dropdown_menu(move |mut menu, window, cx| {
                        menu = menu.scrollable(true);
                        let Some(entity) = view.upgrade() else {
                            return menu;
                        };
                        let current = entity.read(cx);
                        let Some(group) = group else {
                            return menu;
                        };
                        if current.directory != directory
                            || current.target.as_ref().is_none_or(|target| target.group != group)
                        {
                            return menu;
                        }
                        let busy = current.controls_busy(cx);
                        for target in current.group_targets(group) {
                            let selected = current.target.as_ref() == Some(target);
                            let target = target.clone();
                            let view = view.clone();
                            let directory = directory.clone();
                            menu = menu.item(
                                PopupMenuItem::new(target.path.to_string_lossy().into_owned())
                                    .checked(selected)
                                    .disabled(busy)
                                    .on_click(move |_, window, cx| {
                                        let _ = view.update(cx, |this, cx| {
                                            if this.directory == directory
                                                && this
                                                    .target
                                                    .as_ref()
                                                    .is_some_and(|current| current.group == group)
                                            {
                                                this.switch_file(target.clone(), window, cx);
                                            }
                                        });
                                    }),
                            );
                        }
                        menu.max_w(window.rem_size() * 32.)
                    }),
            )
            .into_any_element()
    }

    fn render_file_navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(target) = self.target.as_ref() else {
            return div().into_any_element();
        };
        let ix = self.group_targets(target.group).position(|candidate| candidate == target);
        let count = self.group_targets(target.group).count();
        let position = ix.map(|ix| (ix + 1).to_string()).unwrap_or_else(|| "—".into());
        let busy = self.controls_busy(cx);
        div()
            .flex()
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .child(
                div()
                    .id("diff-file-position")
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .aria_label(format!(
                        "{}：当前第 {position} 个文件，共 {count} 个",
                        target.group.title()
                    ))
                    .child(format!("{position}/{count}")),
            )
            .child(
                Button::new("previous-diff-file")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronLeft)
                    .accessibility_label("上一个 diff 文件")
                    .tooltip("上一个文件")
                    .disabled(busy || self.adjacent_target(-1).is_none())
                    .on_click(cx.listener(|this, _, window, cx| this.move_file(-1, window, cx))),
            )
            .child(
                Button::new("next-diff-file")
                    .small()
                    .ghost()
                    .compact()
                    .icon(IconName::ChevronRight)
                    .accessibility_label("下一个 diff 文件")
                    .tooltip("下一个文件")
                    .disabled(busy || self.adjacent_target(1).is_none())
                    .on_click(cx.listener(|this, _, window, cx| this.move_file(1, window, cx))),
            )
            .into_any_element()
    }

    fn render_file_summary(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = self.state.read(cx);
        let Some(file) = state.files().first().filter(|_| !self.loading && self.error.is_none())
        else {
            return div().into_any_element();
        };
        let status = match file.status() {
            DiffFileStatus::Added => Some("新增"),
            DiffFileStatus::Deleted => Some("删除"),
            DiffFileStatus::Renamed => Some("重命名"),
            DiffFileStatus::Copied => Some("复制"),
            DiffFileStatus::Conflicted => Some("冲突"),
            _ => None,
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .flex_shrink_0()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .when(
                !matches!(file.status(), DiffFileStatus::Unchanged | DiffFileStatus::Conflicted),
                |summary| summary.child(format!("+{} −{}", file.additions(), file.deletions())),
            )
            .children(status)
            .into_any_element()
    }

    fn set_mode(&mut self, mode: DiffMode, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.set_mode(mode, cx));
    }
}

impl Render for GitDiffView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(target) = self.target.as_ref() else {
            return div().into_any_element();
        };
        let path = target.path.to_string_lossy().into_owned();
        let title = self
            .state
            .read(cx)
            .files()
            .first()
            .and_then(|file| match (file.original_path(), file.modified_path()) {
                (Some(old), Some(new)) if old != new => Some(format!("{old} → {new}")),
                _ => None,
            })
            .unwrap_or_else(|| path.clone());
        let toolbar_snapshot = self.tool_snapshot();
        let mode = self.state.read(cx).mode();
        let has_diff = self.error.is_none() && !self.state.read(cx).files().is_empty();
        let body = if let Some(error) = &self.error {
            div()
                .p_4()
                .text_sm()
                .text_color(cx.theme().danger)
                .whitespace_normal()
                .child(format!("无法读取 diff：{error}"))
                .into_any_element()
        } else if self.state.read(cx).files().is_empty() {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(if self.loading {
                    "正在读取 diff"
                } else {
                    "当前比较范围内无变更"
                })
                .into_any_element()
        } else {
            let view = cx.weak_entity();
            let menu_view = view.clone();
            div()
                .id("git-diff-selection-menu")
                .size_full()
                .min_w_0()
                .min_h_0()
                .child(
                    Diff::new(&self.state)
                        .header_visible(false)
                        .decorations(self.decorations.clone())
                        .on_line_click(move |position, _, _, cx| {
                            let _ = view.update(cx, |this, cx| {
                                this.set_location(position.clone(), cx);
                            });
                        })
                        .size_full(),
                )
                .context_menu(move |menu, _, cx| {
                    if let Some(view) = menu_view.upgrade() {
                        view.update(cx, |this, cx| this.selection_menu(menu, cx))
                    } else {
                        menu
                    }
                })
                .into_any_element()
        };
        div()
            .id("git-diff-view")
            .key_context("GitDiff")
            .when(!has_diff, |view| view.track_focus(&self.state.focus_handle(cx)))
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Search, window, cx| {
                    this.show_search(window, cx);
                    cx.stop_propagation();
                },
            ))
            .capture_action(cx.listener(
                |this, action: &gpui_kit::component::input::Enter, window, cx| {
                    if !action.secondary
                        && this.search_open
                        && this
                            .search_input
                            .as_ref()
                            .is_some_and(|input| input.focus_handle(cx).is_focused(window))
                    {
                        this.move_match(action.shift, cx);
                        cx.stop_propagation();
                    }
                },
            ))
            .capture_action(cx.listener(
                |this, _: &gpui_kit::component::input::Escape, window, cx| {
                    if this.search_open {
                        this.close_search(window, cx);
                    } else {
                        this.close(window, cx);
                    }
                    cx.stop_propagation();
                },
            ))
            .child(self.render_groups(cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_wrap()
                    .flex_shrink_0()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border.opacity(0.5))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .flex_1()
                            .min_w(rems(12.))
                            .child(self.render_file_selector(&title, cx))
                            .child(
                                div().flex().items_center().flex_shrink_0().child(
                                    Clipboard::new(SharedString::from(format!(
                                        "copy-diff-path:{path}"
                                    )))
                                    .small()
                                    .value(path)
                                    .accessibility_label("复制文件路径")
                                    .tooltip("复制文件路径"),
                                ),
                            ),
                    )
                    .child(self.render_file_summary(cx))
                    .child(self.render_file_navigation(cx))
                    .when(self.loading, |toolbar| toolbar.child(Spinner::new().small()))
                    .child(
                        Button::new("refresh-diff")
                            .small()
                            .ghost()
                            .compact()
                            .icon(IconName::RefreshCw)
                            .disabled(self.controls_busy(cx))
                            .accessibility_label("刷新 diff")
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        Button::new("close-diff")
                            .small()
                            .ghost()
                            .compact()
                            .icon(IconName::X)
                            .accessibility_label("收起 diff")
                            .tooltip("收起 diff · Esc")
                            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .flex_shrink_0()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border.opacity(0.5))
                    .capture_any_mouse_down(cx.listener(
                        move |this, event: &MouseDownEvent, _, cx| {
                            if event.button == MouseButton::Left {
                                this.restore_tool_snapshot(&toolbar_snapshot, cx);
                            }
                        },
                    ))
                    .child(
                        Button::new("unified-diff")
                            .small()
                            .ghost()
                            .compact()
                            .icon(IconName::Square)
                            .accessibility_label("单栏")
                            .tooltip("单栏")
                            .selected(mode == DiffMode::Unified)
                            .when(mode == DiffMode::Unified, |button| {
                                button
                                    .bg(cx.theme().foreground.opacity(0.08))
                                    .text_color(cx.theme().foreground)
                            })
                            .on_click(
                                cx.listener(|this, _, _, cx| this.set_mode(DiffMode::Unified, cx)),
                            ),
                    )
                    .child(
                        Button::new("split-diff")
                            .small()
                            .ghost()
                            .compact()
                            .icon(IconName::Columns2)
                            .accessibility_label("左右对照")
                            .tooltip("左右对照")
                            .selected(mode == DiffMode::Split)
                            .when(mode == DiffMode::Split, |button| {
                                button
                                    .bg(cx.theme().foreground.opacity(0.08))
                                    .text_color(cx.theme().foreground)
                            })
                            .on_click(
                                cx.listener(|this, _, _, cx| this.set_mode(DiffMode::Split, cx)),
                            ),
                    )
                    .flex_wrap()
                    .child(self.render_write_action(cx))
                    .child(self.render_tools(cx)),
            )
            .when(self.search_open, |view| view.child(self.render_search(cx)))
            .when_some(self.tool_error.clone(), |view, error| {
                view.child(div().px_3().py_1().text_sm().text_color(cx.theme().danger).child(error))
            })
            .child(div().flex_1().min_w_0().min_h_0().child(body))
            .into_any_element()
    }
}
