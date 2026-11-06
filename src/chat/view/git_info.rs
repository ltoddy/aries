pub(crate) mod motion;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use git2::{DiffFindOptions, DiffOptions, Patch, Repository, Status, StatusOptions};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::git_index::{IndexAction, update_index};
use crate::chat::view::git_info::motion::accept_result;

pub fn render_git_info(
    view: &Entity<ChatView>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    view.update(cx, |chat, cx| {
        chat.git_motion.cleanup(cx.background_executor().now(), cx.reduce_motion());
    });
    let chat = view.read(cx);
    if chat.git_info.is_none() && chat.git_read_error.is_none() {
        return None;
    }
    let summary = chat
        .git_read_error
        .as_ref()
        .map(|error| format!("无法读取 Git：{error}"))
        .unwrap_or_else(|| chat.git_info.as_ref().map(GitInfo::summary).unwrap_or_default());
    let now = cx.background_executor().now();
    let expanded = !chat.git_info_collapsed;
    let progress = chat.git_motion.panel.sample(now);
    if chat.git_motion.panel.active(now)
        || chat.git_motion.summary.active(now)
        || chat.git_motion.rows.iter().any(|row| {
            row.visibility.active(now) || row.status.active(now) || row.lines.active(now)
        })
    {
        window.request_animation_frame();
    }
    let id: SharedString =
        format!("project-git-{:?}-{}", view.entity_id(), chat.cwd.display()).into();
    let ball = if progress < 1. || !expanded {
        let view = view.clone();
        Some(
            div()
                .id((ElementId::from(id.clone()), "ball"))
                .absolute()
                .top_3()
                .right_3()
                .rounded_full()
                .shadow_lg()
                .opacity(1. - progress)
                .when(!expanded, |ball| ball.occlude())
                .child(if expanded {
                    div()
                        .size_10()
                        .rounded_full()
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().popover)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(IconName::GitBranch).size_4())
                        .into_any_element()
                } else {
                    Button::new("expand-git-info")
                        .outline()
                        .size_10()
                        .rounded_full()
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().popover_foreground)
                        .icon(IconName::GitBranch)
                        .track_focus(&chat.git_trigger_focus)
                        .disabled(expanded)
                        .accessibility_label("展开 Git 变更")
                        .tooltip(format!("展开 Git 变更 · {summary}"))
                        .on_click(move |_, window, cx| {
                            view.update(cx, |this, cx| this.set_git_collapsed(false, window, cx));
                        })
                        .into_any_element()
                })
                .into_any_element(),
        )
    } else {
        None
    };
    if progress == 0. && !expanded {
        return ball;
    }
    let mut panel = div()
        .id(id)
        .absolute()
        .top_3()
        .right_3()
        .w(rems(20.))
        .max_w(relative(0.9))
        .p_3()
        .rounded_lg()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .text_color(cx.theme().popover_foreground)
        .shadow_lg()
        .opacity(progress)
        .when(expanded, |panel| panel.occlude())
        .mt(cx.theme().motion_tokens().distance_short * (1. - progress))
        .flex()
        .flex_col()
        .gap_3()
        .min_w_0()
        .aria_label("Git 变更文件")
        .when(expanded, |panel| {
            let view = view.clone();
            panel.on_key_down(move |event, window, cx| {
                if event.keystroke.key == "escape" {
                    view.update(cx, |this, cx| this.set_git_collapsed(true, window, cx));
                    cx.stop_propagation();
                }
            })
        })
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Git 变更"))
                .child(if !expanded {
                    div()
                        .size_6()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(IconName::ChevronUp).size_4())
                        .into_any_element()
                } else {
                    let view = view.clone();
                    let label = "缩小为悬浮球";
                    Button::new("toggle-git-info")
                        .track_focus(&chat.git_panel_focus)
                        .disabled(!expanded)
                        .ghost()
                        .small()
                        .icon(IconName::ChevronUp)
                        .accessibility_label(label)
                        .tooltip(label)
                        .on_click(move |_, window, cx| {
                            view.update(cx, |this, cx| this.set_git_collapsed(true, window, cx));
                        })
                        .into_any_element()
                }),
        );
    let summary_opacity = chat.git_motion.summary.sample(now);
    panel = panel.child(
        div()
            .relative()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(div().whitespace_normal().opacity(summary_opacity).child(summary))
            .when_some(chat.git_motion.previous_summary.as_ref(), |summary, previous| {
                summary.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .w_full()
                        .whitespace_normal()
                        .opacity(1. - summary_opacity)
                        .child(previous.clone()),
                )
            }),
    );
    if !chat.git_motion.rows.is_empty() {
        panel = panel
            .child(render_change_group(view, ChangeGroup::Staged, window.rem_size() * 2., cx))
            .child(render_change_group(view, ChangeGroup::Unstaged, window.rem_size() * 2., cx));
    }
    Some(
        div()
            .children([Some(panel.into_any_element()), ball].into_iter().flatten())
            .into_any_element(),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChangeGroup {
    Staged,
    Unstaged,
}

impl ChangeGroup {
    fn id(self) -> &'static str {
        match self {
            Self::Staged => "git-staged",
            Self::Unstaged => "git-unstaged",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Staged => "已暂存",
            Self::Unstaged => "未暂存",
        }
    }

    fn action(self) -> IndexAction {
        match self {
            Self::Staged => IndexAction::Unstage,
            Self::Unstaged => IndexAction::Stage,
        }
    }
}

fn render_change_group(
    view: &Entity<ChatView>,
    group: ChangeGroup,
    row_height: Pixels,
    cx: &App,
) -> AnyElement {
    let chat = view.read(cx);
    let count = chat.git_motion.rows(group).count();
    let live_count = chat.git_motion.rows(group).filter(|row| !row.exiting).count();
    let mut panel = div().id(group.id()).flex().flex_col().gap_1().child(
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!("{} · {live_count}", group.title())),
    );
    if count == 0 {
        return panel.into_any_element();
    }
    let scroll = &chat.git_motion.scroll[match group {
        ChangeGroup::Staged => 0,
        ChangeGroup::Unstaged => 1,
    }];
    if chat.git_info_collapsed {
        let now = cx.background_executor().now();
        let offset = scroll.0.borrow().base_handle.offset().y;
        let start = (-offset / row_height).floor() as usize;
        let fraction = offset + row_height * start as f32;
        return panel
            .child(div().h(row_height * count.min(5) as f32).overflow_hidden().child(
                div().relative().top(fraction).children(
                    chat.git_motion.rows(group).skip(start).take(6).map(|row| {
                        div()
                            .h_8()
                            .w_full()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .opacity(row.visibility.sample(now))
                            .child(
                                div()
                                    .opacity(row.status.sample(now))
                                    .flex_shrink_0()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        Icon::new(ChangedFile::status_icon(row.file.status))
                                            .size_4(),
                                    ),
                            )
                            .child(
                                div()
                                    .size_6()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(
                                        Icon::new(match group.action() {
                                            IndexAction::Stage => IconName::Plus,
                                            IndexAction::Unstage => IconName::Minus,
                                        })
                                        .size_4(),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .child(row.file.path.to_string_lossy().into_owned()),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .opacity(row.lines.sample(now))
                                    .child(row.file.lines.render(cx)),
                            )
                    }),
                ),
            ))
            .into_any_element();
    }
    let view = view.clone();
    panel = panel.child(
        uniform_list(group.id(), count, move |range, _, cx| {
            let chat = view.read(cx);
            let now = cx.background_executor().now();
            let directory = &chat.cwd;
            let busy = chat.git_index_busy;
            let start = range.start;
            let length = range.len();
            chat.git_motion
                .rows(group)
                .skip(start)
                .take(length)
                .map(|row| {
                    let file = &row.file;
                    let loading = chat.git_pending_index.as_ref().is_some_and(|(path, action)| {
                        path == &file.path && *action == group.action()
                    });
                    let path = SharedString::from(file.path.to_string_lossy().into_owned());
                    let status = ChangedFile::status_kind(file.status);
                    let action = group.action();
                    let action_label = match action {
                        IndexAction::Stage => "添加到暂存区",
                        IndexAction::Unstage => "取消暂存",
                    };
                    let view = view.clone();
                    let target = file.path.clone();
                    let directory = directory.clone();
                    div()
                        .id(path.clone())
                        .h_8()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded_md()
                        .opacity(row.visibility.sample(now))
                        .when(!row.exiting && !busy, |row| {
                            row.hover(|style| style.bg(cx.theme().accent))
                        })
                        .aria_label(format!("{path} · {status}"))
                        .child(
                            div()
                                .id("file-status")
                                .opacity(row.status.sample(now))
                                .flex_shrink_0()
                                .text_color(cx.theme().muted_foreground)
                                .child(Icon::new(ChangedFile::status_icon(file.status)).size_4())
                                .tooltip(move |window, cx| Tooltip::new(status).build(window, cx)),
                        )
                        .child(
                            Button::new("index-action")
                                .flex_shrink_0()
                                .ghost()
                                .small()
                                .size_6()
                                .loading(loading)
                                .icon(match action {
                                    IndexAction::Stage => IconName::Plus,
                                    IndexAction::Unstage => IconName::Minus,
                                })
                                .accessibility_label(format!(
                                    "{action_label}：{}",
                                    target.display()
                                ))
                                .tooltip(action_label)
                                .disabled(
                                    (busy && !loading)
                                        || row.exiting
                                        || chat.git_info_collapsed
                                        || file.status.is_conflicted(),
                                )
                                .on_click(move |_, window, cx| {
                                    view.update(cx, |this, cx| {
                                        if this.cwd == directory {
                                            this.change_git_index(
                                                target.clone(),
                                                action,
                                                window,
                                                cx,
                                            );
                                        }
                                    });
                                }),
                        )
                        .child(
                            div()
                                .id("file-path")
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_sm()
                                .child(path.clone())
                                .tooltip(move |window, cx| {
                                    Tooltip::new(path.clone()).build(window, cx)
                                }),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .opacity(row.lines.sample(now))
                                .child(file.lines.render(cx)),
                        )
                })
                .collect()
        })
        .track_scroll(scroll)
        .h(rems(count.min(5) as f32 * 2.))
        .w_full(),
    );
    panel.into_any_element()
}

impl ChatView {
    fn set_git_collapsed(&mut self, collapsed: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.git_info_collapsed == collapsed {
            return;
        }
        self.git_info_collapsed = collapsed;
        let tokens = cx.theme().motion_tokens();
        let duration = if cx.reduce_motion() { Duration::ZERO } else { tokens.duration_fast };
        self.git_motion.panel.retarget(
            if collapsed { 0. } else { 1. },
            cx.background_executor().now(),
            duration,
            if collapsed { tokens.easing_exit.clone() } else { tokens.easing_enter.clone() },
        );
        if collapsed {
            self.git_trigger_focus.focus(window, cx);
        } else {
            self.git_panel_focus.focus(window, cx);
        }
        cx.notify();
    }

    pub(crate) fn sync_git_info(
        &mut self,
        result: Result<Option<GitInfo>, String>,
        animate: bool,
        cx: &mut Context<Self>,
    ) {
        if self.git_motion.reset(&self.cwd) {
            self.git_info = None;
            self.git_read_error = None;
            self.git_info_collapsed = true;
        }
        let (info, error) = match result {
            Ok(None) if self.git_info.is_some() || self.git_read_error.is_some() => {
                (None, Some("项目不再是可读取的 Git 仓库".into()))
            }
            Ok(info) => (info, None),
            Err(error) => (None, Some(error)),
        };
        let changed = self.git_info != info || self.git_read_error != error;
        let tokens = cx.theme().motion_tokens();
        let duration = if cx.reduce_motion() { Duration::ZERO } else { tokens.duration_fast };
        self.git_motion.sync(
            info.as_ref(),
            cx.background_executor().now(),
            duration,
            tokens.easing_enter.clone(),
            animate,
        );
        self.git_info = info;
        self.git_read_error = error;
        if changed {
            if !duration.is_zero() {
                cx.spawn(async move |view, cx| {
                    cx.background_executor().timer(duration).await;
                    let _ = view.update(cx, |this, cx| {
                        this.git_motion.cleanup(cx.background_executor().now(), cx.reduce_motion());
                        cx.notify();
                    });
                })
                .detach();
            }
            cx.notify();
        }
    }

    fn change_git_index(
        &mut self,
        path: PathBuf,
        action: IndexAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let group = match action {
            IndexAction::Stage => ChangeGroup::Unstaged,
            IndexAction::Unstage => ChangeGroup::Staged,
        };
        if self.git_index_busy
            || self.git_info_collapsed
            || !self.git_motion.rows(group).any(|row| {
                row.file.path == path && !row.exiting && !row.file.status.is_conflicted()
            })
        {
            return;
        }
        self.git_index_busy = true;
        self.git_pending_index = Some((path.clone(), action));
        self.git_revision += 1;
        let revision = self.git_revision;
        let directory = self.cwd.clone();
        let action_focus = window.focused(cx);
        cx.spawn_in(window, async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let result = update_index(&directory, &path, action);
                    let info = GitInfo::read(&directory);
                    (result, info, path, directory)
                })
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.git_index_busy = false;
                this.git_pending_index = None;
                let current = accept_result(&this.cwd, this.git_revision, &result.3, revision);
                if current {
                    this.sync_git_info(result.1, result.0.is_ok(), cx);
                    if !this.git_info_collapsed
                        && window.focused(cx) == action_focus
                        && !this
                            .git_motion
                            .rows(group)
                            .any(|row| row.file.path == result.2 && !row.exiting)
                    {
                        this.git_panel_focus.focus(window, cx);
                    }
                }
                this.git_revision += 1;
                if let Err(error) = result.0 {
                    let label = match action {
                        IndexAction::Stage => "无法暂存",
                        IndexAction::Unstage => "无法取消暂存",
                    };
                    window.push_notification(
                        Notification::error(format!("{label} {}：{error}", result.2.display())),
                        cx,
                    );
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileChange {
    path: PathBuf,
    status: Status,
    lines: LineChanges,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineChanges {
    Text { added: usize, deleted: usize },
    Binary,
    Unavailable,
}

impl LineChanges {
    fn render(self, cx: &App) -> AnyElement {
        let row = div().id("line-changes").flex_shrink_0().flex().items_center().gap_1().text_xs();
        match self {
            Self::Text { added, deleted } => row
                .aria_label(format!("新增 {added} 行，删除 {deleted} 行"))
                .child(div().text_color(cx.theme().success).child(format!("+{added}")))
                .child(div().text_color(cx.theme().danger).child(format!("−{deleted}"))),
            Self::Binary => row.text_color(cx.theme().muted_foreground).child("二进制"),
            Self::Unavailable => row.text_color(cx.theme().muted_foreground).child("统计不可用"),
        }
        .into_any_element()
    }
}

fn read_line_changes(
    repository: &Repository,
    group: ChangeGroup,
) -> Result<BTreeMap<PathBuf, LineChanges>, git2::Error> {
    let mut options = DiffOptions::new();
    options.include_untracked(true).recurse_untracked_dirs(true).show_untracked_content(true);
    let mut diff = match group {
        ChangeGroup::Staged => {
            let tree = match repository.head() {
                Ok(head) => Some(head.peel_to_tree()?),
                Err(error) if error.code() == git2::ErrorCode::UnbornBranch => None,
                Err(error) => return Err(error),
            };
            repository.diff_tree_to_index(tree.as_ref(), None, Some(&mut options))?
        }
        ChangeGroup::Unstaged => repository.diff_index_to_workdir(None, Some(&mut options))?,
    };
    let mut find = DiffFindOptions::new();
    find.renames(true).for_untracked(true);
    diff.find_similar(Some(&mut find))?;
    let mut changes = BTreeMap::new();
    for ix in 0..diff.deltas().len() {
        let lines = match Patch::from_diff(&diff, ix) {
            Ok(Some(patch)) if patch.delta().flags().is_binary() => LineChanges::Binary,
            Ok(Some(patch)) => match patch.line_stats() {
                Ok((_, added, deleted)) => LineChanges::Text { added, deleted },
                Err(_) => LineChanges::Unavailable,
            },
            Ok(None) => {
                if diff.get_delta(ix).is_some_and(|delta| delta.flags().is_binary()) {
                    LineChanges::Binary
                } else {
                    LineChanges::Text { added: 0, deleted: 0 }
                }
            }
            Err(_) => LineChanges::Unavailable,
        };
        if let Some(delta) = diff.get_delta(ix) {
            for path in [delta.old_file().path(), delta.new_file().path()].into_iter().flatten() {
                changes.insert(path.to_owned(), lines);
            }
        }
    }
    Ok(changes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    path: String,
    status: Status,
    staged: Option<FileChange>,
    unstaged: Option<FileChange>,
}

impl ChangedFile {
    fn change(&self, group: ChangeGroup) -> Option<&FileChange> {
        match group {
            ChangeGroup::Staged => self.staged.as_ref(),
            ChangeGroup::Unstaged => self.unstaged.as_ref(),
        }
    }

    fn status_icon(status: Status) -> IconName {
        if status.is_conflicted() {
            IconName::FileExclamationPoint
        } else if status.intersects(Status::INDEX_RENAMED | Status::WT_RENAMED) {
            IconName::FileOutput
        } else if status.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
            IconName::FileMinus
        } else if status.is_wt_new() {
            IconName::FileQuestionMark
        } else if status.is_index_new() {
            IconName::FilePlusCorner
        } else if status.intersects(Status::INDEX_TYPECHANGE | Status::WT_TYPECHANGE) {
            IconName::FileType
        } else {
            IconName::FilePen
        }
    }

    fn status_kind(status: Status) -> &'static str {
        if status.is_conflicted() {
            "冲突"
        } else if status.intersects(Status::INDEX_RENAMED | Status::WT_RENAMED) {
            "重命名"
        } else if status.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
            "删除"
        } else if status.is_wt_new() {
            "未跟踪"
        } else if status.is_index_new() {
            "新增"
        } else if status.intersects(Status::INDEX_TYPECHANGE | Status::WT_TYPECHANGE) {
            "类型变更"
        } else {
            "修改"
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitInfo {
    branch: String,
    changed_files: Option<Vec<ChangedFile>>,
}

impl GitInfo {
    pub fn read(cwd: &Path) -> Result<Option<Self>, String> {
        let repository = match Repository::discover(cwd) {
            Ok(repository) => repository,
            Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(error.to_string()),
        };
        let head = repository.find_reference("HEAD").map_err(|error| error.to_string())?;
        let branch = match head.symbolic_target().map_err(|error| error.to_string())? {
            Some(target) => target.strip_prefix("refs/heads/").unwrap_or(target).to_owned(),
            None => {
                format!("游离 HEAD · {}", &head.target().ok_or("HEAD 没有目标")?.to_string()[..7])
            }
        };
        let mut options = StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);
        let staged_lines = read_line_changes(&repository, ChangeGroup::Staged).ok();
        let unstaged_lines = read_line_changes(&repository, ChangeGroup::Unstaged).ok();
        let changed_files = Some({
            let statuses =
                repository.statuses(Some(&mut options)).map_err(|error| error.to_string())?;
            let mut files: Vec<_> = statuses
                .iter()
                .map(|entry| {
                    let staged_flags = Status::INDEX_NEW
                        | Status::INDEX_MODIFIED
                        | Status::INDEX_DELETED
                        | Status::INDEX_RENAMED
                        | Status::INDEX_TYPECHANGE;
                    let unstaged_flags = Status::WT_NEW
                        | Status::WT_MODIFIED
                        | Status::WT_DELETED
                        | Status::WT_RENAMED
                        | Status::WT_TYPECHANGE
                        | Status::CONFLICTED;
                    let change =
                        |delta: git2::DiffDelta<'_>,
                         status: Status,
                         lines: Option<&BTreeMap<PathBuf, LineChanges>>| {
                            delta.new_file().path().or_else(|| delta.old_file().path()).map(
                                |path| FileChange {
                                    path: path.to_owned(),
                                    status,
                                    lines: if status.is_conflicted() {
                                        LineChanges::Unavailable
                                    } else {
                                        lines
                                            .and_then(|lines| lines.get(path))
                                            .copied()
                                            .unwrap_or(LineChanges::Unavailable)
                                    },
                                },
                            )
                        };
                    ChangedFile {
                        path: String::from_utf8_lossy(entry.path_bytes()).into_owned(),
                        status: entry.status(),
                        staged: entry.head_to_index().and_then(|delta| {
                            change(
                                delta,
                                entry.status() & (staged_flags | Status::CONFLICTED),
                                staged_lines.as_ref(),
                            )
                        }),
                        unstaged: entry.index_to_workdir().and_then(|delta| {
                            change(delta, entry.status() & unstaged_flags, unstaged_lines.as_ref())
                        }),
                    }
                })
                .collect();
            files.sort_by(|left, right| left.path.cmp(&right.path));
            files
        });
        Ok(Some(Self { branch, changed_files }))
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn changed_files(&self) -> Option<&[ChangedFile]> {
        self.changed_files.as_deref()
    }

    pub fn summary(&self) -> String {
        let branch = &self.branch;
        match self.changed_files.as_deref() {
            Some([]) => format!("{branch} · 工作区干净"),
            Some(files) => format!("{branch} · {} 个文件变更", files.len()),
            None => format!("{branch} · 无法读取变更"),
        }
    }
}
