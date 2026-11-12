use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::diff::{Diff, DiffFile, DiffMode, DiffState};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::git::index::IndexAction;
use crate::workspace::git::repository::{ChangeGroup, read_file_diff};

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
        let subscription = cx.observe(&state, |_, _, cx| cx.notify());
        Self {
            owner,
            directory,
            target: None,
            state,
            patch: Arc::new(String::new()),
            error: None,
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
            cx.emit(DiffTargetChanged::new());
        }
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
                        return Ok::<_, anyhow::Error>(None);
                    }
                    let files = DiffFile::parse(&patch)?;
                    Ok(Some((Arc::new(patch), files)))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.revision != revision {
                    return;
                }
                this.loading = false;
                match result {
                    Ok(result) => {
                        if let Some((patch, files)) = result {
                            this.state.update(cx, |state, cx| state.set_files(files, cx));
                            this.patch = patch;
                        }
                        this.error = None;
                    },
                    Err(error) => this.error = Some(format!("{error:#}")),
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

    fn render_index_actions(&self, path: &Path, cx: &App) -> AnyElement {
        let Some(owner) = self.owner.upgrade() else {
            return div().into_any_element();
        };
        let panel = owner.read(cx).git_panel();
        let mut actions = div().flex().items_center().flex_shrink_0().gap_1();
        for (id, action, label, icon) in [
            ("stage-diff", IndexAction::Stage, "添加到暂存区", IconName::Plus),
            ("unstage-diff", IndexAction::Unstage, "移出暂存区", IconName::Minus),
        ] {
            let loading = panel.pending_index().is_some_and(|(pending_path, pending_action)| {
                pending_path == path && *pending_action == action
            });
            let available = panel.directory() == self.directory
                && panel.is_index_action_available(path, action);
            let owner = self.owner.clone();
            let path = path.to_owned();
            let directory = self.directory.clone();
            actions = actions.child(
                Button::new(id)
                    .small()
                    .ghost()
                    .icon(icon)
                    .loading(loading)
                    .disabled(!available && !loading)
                    .accessibility_label(format!("{label}：{}", path.display()))
                    .tooltip(format!("{label}（整个文件）"))
                    .on_click(move |_, window, cx| {
                        let owner = owner.clone();
                        let path = path.clone();
                        let directory = directory.clone();
                        window.defer(cx, move |window, cx| {
                            let _ = owner.update(cx, |session, cx| {
                                session.with_git_panel(|panel| {
                                    if panel.directory() == directory {
                                        panel.change_git_index(path, action, window, cx);
                                    }
                                });
                            });
                        });
                    }),
            );
        }
        actions.into_any_element()
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
                .text_color(cx.theme().muted_foreground)
                .child(if self.loading {
                    "正在读取 diff"
                } else {
                    "当前比较范围内无变更"
                })
                .into_any_element()
        } else {
            Diff::new(&self.state).size_full().into_any_element()
        };
        div()
            .id("git-diff-view")
            .when(!has_diff, |view| view.track_focus(&self.state.focus_handle(cx)))
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                if event.keystroke.key == "escape" {
                    this.close(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .flex_shrink_0()
                    .px_3()
                    .py_2()
                    .border_b_1()
                    .border_color(cx.theme().border)
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
                            )
                            .child(self.render_index_actions(&target.path, cx)),
                    )
                    .when(self.loading, |toolbar| toolbar.child(Spinner::new().small()))
                    .child(
                        Button::new("refresh-diff")
                            .small()
                            .ghost()
                            .icon(IconName::RefreshCw)
                            .disabled(self.loading)
                            .accessibility_label("刷新 diff")
                            .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
                    )
                    .child(
                        Button::new("close-diff")
                            .small()
                            .ghost()
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
                    .child(
                        Button::new("unified-diff")
                            .small()
                            .ghost()
                            .icon(IconName::Square)
                            .accessibility_label("单栏")
                            .tooltip("单栏")
                            .selected(mode == DiffMode::Unified)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.set_mode(DiffMode::Unified, cx)),
                            ),
                    )
                    .child(
                        Button::new("split-diff")
                            .small()
                            .ghost()
                            .icon(IconName::Columns2)
                            .accessibility_label("左右对照")
                            .tooltip("左右对照")
                            .selected(mode == DiffMode::Split)
                            .on_click(
                                cx.listener(|this, _, _, cx| this.set_mode(DiffMode::Split, cx)),
                            ),
                    ),
            )
            .child(div().flex_1().min_w_0().min_h_0().child(body))
            .into_any_element()
    }
}
