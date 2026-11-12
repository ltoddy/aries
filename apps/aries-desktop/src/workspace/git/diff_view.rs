use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::diff::{
    Diff, DiffDecoration, DiffFile, DiffLinePosition, DiffLineRange, DiffMode, DiffState,
};
use gpui_kit::component::input::InputState;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use self::tools::ToolScope;
use crate::error::details;
use crate::workspace::SessionView;
use crate::workspace::git::diff_operations::DiscardBackupEntry;
use crate::workspace::git::diff_patch::{DiffPatch, PatchScope};
use crate::workspace::git::error::DiffLoadError;
use crate::workspace::git::repository::{ChangeGroup, read_file_diff};

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
    state: Entity<DiffState>,
    patch: Arc<String>,
    error: Option<String>,
    model: Option<Arc<DiffPatch>>,
    tool_error: Option<String>,
    scope: ToolScope,
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
    editor_task: Option<Task<()>>,
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
            state,
            patch: Arc::new(String::new()),
            error: None,
            model: None,
            tool_error: None,
            scope: ToolScope::file(),
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
            editor_task: None,
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
        let (comparison, source_icon, destination_icon) = match target.group {
            ChangeGroup::Staged => {
                ("已暂存 · HEAD → 暂存区", IconName::GitCommitHorizontal, IconName::Layers)
            },
            ChangeGroup::Unstaged => {
                ("未暂存 · 暂存区 → 工作目录", IconName::Layers, IconName::Folder)
            },
        };
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
            Diff::new(&self.state)
                .decorations(self.decorations.clone())
                .on_line_click(move |position, _, _, cx| {
                    let _ = view.update(cx, |this, cx| this.set_location(position.clone(), cx));
                })
                .size_full()
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
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_shrink_0()
                    .px_3()
                    .py_1()
                    .border_b_1()
                    .border_color(cx.theme().border.opacity(0.5))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().min_w_0().text_sm().truncate().child(path.clone()))
                                    .child(
                                        Clipboard::new(SharedString::from(format!(
                                            "copy-diff-path:{path}"
                                        )))
                                        .small()
                                        .value(path)
                                        .accessibility_label("复制文件路径")
                                        .tooltip("复制文件路径"),
                                    ),
                            )
                            .child(
                                div()
                                    .id("diff-comparison")
                                    .flex()
                                    .flex_shrink_0()
                                    .items_center()
                                    .gap_1()
                                    .text_color(cx.theme().muted_foreground)
                                    .aria_label(comparison)
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(comparison).build(window, cx)
                                    })
                                    .child(Icon::new(source_icon).size_3())
                                    .child(Icon::new(IconName::ArrowRight).size_3())
                                    .child(Icon::new(destination_icon).size_3()),
                            ),
                    )
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
