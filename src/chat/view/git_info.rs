use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

pub fn render_git_info(view: Entity<ChatView>, cx: &App) -> Option<AnyElement> {
    let chat = view.read(cx);
    let info = chat.git_info.as_ref()?;
    let id: SharedString =
        format!("project-git-{:?}-{}", view.entity_id(), chat.cwd.display()).into();
    if chat.git_info_collapsed {
        let summary = info.summary();
        return Some(
            div()
                .id(id)
                .absolute()
                .top_3()
                .right_3()
                .rounded_full()
                .shadow_lg()
                .occlude()
                .child(
                    Button::new("expand-git-info")
                        .outline()
                        .size_10()
                        .rounded_full()
                        .bg(cx.theme().popover)
                        .text_color(cx.theme().popover_foreground)
                        .icon(IconName::GitBranch)
                        .accessibility_label("展开 Git 变更")
                        .tooltip(format!("展开 Git 变更 · {summary}"))
                        .on_click(move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                this.git_info_collapsed = false;
                                cx.notify();
                            });
                        }),
                )
                .into_any_element(),
        );
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
        .occlude()
        .flex()
        .flex_col()
        .gap_3()
        .min_w_0()
        .aria_label("Git 变更文件")
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Git 变更"))
                .child({
                    let view = view.clone();
                    let label = "缩小为悬浮球";
                    Button::new("toggle-git-info")
                        .ghost()
                        .small()
                        .icon(IconName::ChevronUp)
                        .accessibility_label(label)
                        .tooltip(label)
                        .on_click(move |_, _, cx| {
                            view.update(cx, |this, cx| {
                                this.git_info_collapsed = true;
                                cx.notify();
                            });
                        })
                }),
        );
    panel = panel.child(
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .whitespace_normal()
            .child(info.summary()),
    );
    Some(
        match info.changed_files() {
            Some([]) => panel,
            None => panel.child(div().text_sm().child("无法读取文件变更，请检查项目目录权限。")),
            Some(_) => panel
                .when(chat.git_index_busy, |panel| {
                    panel.child(div().text_xs().child("正在更新暂存区…"))
                })
                .child(render_change_group(view.clone(), ChangeGroup::Staged, cx))
                .child(render_change_group(view.clone(), ChangeGroup::Unstaged, cx)),
        }
        .into_any_element(),
    )
}

#[derive(Clone, Copy)]
enum ChangeGroup {
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

fn render_change_group(view: Entity<ChatView>, group: ChangeGroup, cx: &App) -> AnyElement {
    let chat = view.read(cx);
    let count = chat
        .git_info
        .as_ref()
        .and_then(GitInfo::changed_files)
        .unwrap_or_default()
        .iter()
        .filter(|file| file.change(group).is_some())
        .count();
    let mut panel = div().id(group.id()).flex().flex_col().gap_1().child(
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!("{} · {count}", group.title())),
    );
    if count == 0 {
        return panel.into_any_element();
    }
    panel = panel.child(
        uniform_list(group.id(), count, move |range, _, cx| {
            let chat = view.read(cx);
            let files = chat.git_info.as_ref().and_then(GitInfo::changed_files).unwrap_or_default();
            let directory = chat.cwd.clone();
            let busy = chat.git_index_busy;
            let start = range.start;
            let length = range.len();
            files
                .iter()
                .filter_map(|file| file.change(group))
                .skip(start)
                .take(length)
                .map(|file| {
                    let path = file.path.to_string_lossy().into_owned();
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
                        .id(SharedString::from(path.clone()))
                        .h_8()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_2()
                        .rounded_md()
                        .hover(|style| style.bg(cx.theme().accent))
                        .aria_label(format!("{path} · {status}"))
                        .child(
                            div()
                                .id("file-status")
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
                                .icon(match action {
                                    IndexAction::Stage => IconName::Plus,
                                    IndexAction::Unstage => IconName::Minus,
                                })
                                .accessibility_label(format!(
                                    "{action_label}：{}",
                                    target.display()
                                ))
                                .tooltip(action_label)
                                .disabled(busy || file.status.is_conflicted())
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
                        .child(file.lines.render(cx))
                })
                .collect()
        })
        .h(rems(count.min(5) as f32 * 2.))
        .w_full(),
    );
    panel.into_any_element()
}

impl ChatView {
    fn change_git_index(
        &mut self,
        path: PathBuf,
        action: IndexAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.git_index_busy {
            return;
        }
        self.git_index_busy = true;
        self.git_revision += 1;
        let directory = self.cwd.clone();
        cx.spawn_in(window, async move |view, cx| {
            let cwd = directory.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    let result = update_index(&cwd, &path, action);
                    (result, GitInfo::read(&cwd), path)
                })
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.git_index_busy = false;
                this.git_revision += 1;
                if this.cwd == directory {
                    this.git_info = result.1;
                }
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
struct FileChange {
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

    #[cfg(test)]
    pub fn path(&self) -> &str {
        &self.path
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

    #[cfg(test)]
    fn status_label(&self) -> String {
        let status = self.status;
        let kind = Self::status_kind(status);
        let staged = status.intersects(
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE,
        );
        let unstaged = status.intersects(
            Status::WT_MODIFIED | Status::WT_DELETED | Status::WT_RENAMED | Status::WT_TYPECHANGE,
        );
        match (staged, unstaged) {
            (true, true) => format!("{kind} · 部分暂存"),
            (true, false) => format!("{kind} · 已暂存"),
            _ => kind.to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitInfo {
    branch: String,
    changed_files: Option<Vec<ChangedFile>>,
}

impl GitInfo {
    pub fn read(cwd: &Path) -> Option<Self> {
        let repository = Repository::discover(cwd).ok()?;
        let head = repository.find_reference("HEAD").ok()?;
        let branch = match head.symbolic_target().ok()? {
            Some(target) => target.strip_prefix("refs/heads/")?.to_owned(),
            None => format!("游离 HEAD · {}", &head.target()?.to_string()[..7]),
        };
        let mut options = StatusOptions::new();
        options
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);
        let staged_lines = read_line_changes(&repository, ChangeGroup::Staged).ok();
        let unstaged_lines = read_line_changes(&repository, ChangeGroup::Unstaged).ok();
        let changed_files = repository.statuses(Some(&mut options)).ok().map(|statuses| {
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
        Some(Self { branch, changed_files })
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use git2::{Repository, RepositoryInitOptions, Signature};

    use crate::chat::view::git_index::{IndexAction, update_index};
    use crate::chat::view::git_info::{ChangeGroup, ChangedFile, GitInfo, LineChanges};

    static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("aries-orbit-git-{}-{id}", std::process::id()));
            fs::create_dir(&path).expect("test directory should be created");
            Self(path)
        }

        fn repository(&self) -> Repository {
            let mut options = RepositoryInitOptions::new();
            options.initial_head("main");
            Repository::init_opts(&self.0, &options).expect("test repository should be initialized")
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn hides_non_repository() {
        let directory = TestDirectory::new();
        assert_eq!(GitInfo::read(&directory.0), None);
    }

    #[test]
    fn reads_unborn_branch() {
        let directory = TestDirectory::new();
        let _repository = directory.repository();
        let info =
            GitInfo::read(&directory.0).expect("empty repository should have Git information");
        assert_eq!(info.branch(), "main");
        assert_eq!(info.changed_files(), Some([].as_slice()));
        assert_eq!(info.summary(), "main · 工作区干净");
    }

    #[test]
    fn counts_staged_and_untracked_files_without_ignored_files() {
        let directory = TestDirectory::new();
        let repository = directory.repository();
        fs::write(directory.0.join(".gitignore"), "ignored\n")
            .expect("ignore file should be written");
        fs::write(directory.0.join("ignored"), "ignored").expect("ignored file should be written");
        fs::create_dir(directory.0.join("nested")).expect("nested directory should be created");
        fs::write(directory.0.join("nested/untracked"), "new")
            .expect("untracked file should be written");
        fs::write(directory.0.join("staged"), "staged").expect("staged file should be written");
        let mut index = repository.index().expect("repository should have an index");
        index.add_path(Path::new("staged")).expect("file should be staged");
        index.write().expect("index should be written");
        fs::write(directory.0.join("staged"), "modified").expect("staged file should be modified");
        let info = GitInfo::read(&directory.0).expect("repository should have Git information");
        let files = info.changed_files().expect("file status should be readable");
        assert_eq!(
            files.iter().map(|file| file.path()).collect::<Vec<_>>(),
            [".gitignore", "nested/untracked", "staged"]
        );
        assert_eq!(files[0].status_label(), "未跟踪");
        assert_eq!(files[2].status_label(), "新增 · 部分暂存");
        assert!(files[0].change(ChangeGroup::Staged).is_none());
        assert!(files[0].change(ChangeGroup::Unstaged).is_some());
        let staged =
            files[2].change(ChangeGroup::Staged).expect("file should appear in staged group");
        let unstaged =
            files[2].change(ChangeGroup::Unstaged).expect("file should appear in unstaged group");
        assert_eq!(ChangedFile::status_kind(staged.status), "新增");
        assert_eq!(ChangedFile::status_kind(unstaged.status), "修改");
        assert_eq!(staged.path, unstaged.path);
    }

    #[test]
    fn counts_lines_separately_for_partial_staging_and_deletion() {
        let directory = TestDirectory::new();
        let repository = directory.repository();
        let path = Path::new("file");
        fs::write(directory.0.join(path), "one\ntwo\nthree\n").expect("file should be written");
        update_index(&directory.0, path, IndexAction::Stage).expect("file should be staged");
        let tree_id = repository
            .index()
            .expect("index should exist")
            .write_tree()
            .expect("tree should be written");
        let tree = repository.find_tree(tree_id).expect("tree should exist");
        let signature =
            Signature::now("Test", "test@example.com").expect("signature should be valid");
        repository
            .commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .expect("commit should exist");
        fs::write(directory.0.join(path), "one\nchanged\nthree\nextra\n")
            .expect("file should be modified");
        update_index(&directory.0, path, IndexAction::Stage).expect("changes should be staged");
        fs::write(directory.0.join(path), "one\nchanged\n").expect("file should be modified again");
        let info = GitInfo::read(&directory.0).expect("Git information should exist");
        let file = &info.changed_files().expect("files should be readable")[0];
        assert_eq!(
            file.change(ChangeGroup::Staged).expect("staged changes should exist").lines,
            LineChanges::Text { added: 2, deleted: 1 }
        );
        assert_eq!(
            file.change(ChangeGroup::Unstaged).expect("unstaged changes should exist").lines,
            LineChanges::Text { added: 0, deleted: 2 }
        );
        fs::remove_file(directory.0.join(path)).expect("file should be deleted");
        let info = GitInfo::read(&directory.0).expect("Git information should exist");
        assert_eq!(
            info.changed_files().expect("files should exist")[0]
                .change(ChangeGroup::Unstaged)
                .expect("deletion should exist")
                .lines,
            LineChanges::Text { added: 0, deleted: 4 }
        );
        update_index(&directory.0, path, IndexAction::Stage).expect("deletion should be staged");
        let info = GitInfo::read(&directory.0).expect("Git information should exist");
        assert_eq!(
            info.changed_files().expect("files should exist")[0]
                .change(ChangeGroup::Staged)
                .expect("deletion should exist")
                .lines,
            LineChanges::Text { added: 0, deleted: 3 }
        );
    }

    #[test]
    fn counts_untracked_and_unborn_files_and_identifies_binary_content() {
        let directory = TestDirectory::new();
        let _repository = directory.repository();
        fs::write(directory.0.join("text"), "one\ntwo").expect("text should be written");
        fs::write(directory.0.join("binary"), b"one\0two\n").expect("binary should be written");
        for group in [ChangeGroup::Unstaged, ChangeGroup::Staged] {
            if matches!(group, ChangeGroup::Staged) {
                for path in ["text", "binary"] {
                    update_index(&directory.0, Path::new(path), IndexAction::Stage)
                        .expect("file should be staged");
                }
            }
            let info = GitInfo::read(&directory.0).expect("Git information should exist");
            let files = info.changed_files().expect("files should be readable");
            assert_eq!(
                files[0].change(group).expect("binary should exist").lines,
                LineChanges::Binary
            );
            assert_eq!(
                files[1].change(group).expect("text should exist").lines,
                LineChanges::Text { added: 2, deleted: 0 }
            );
        }
    }

    #[test]
    fn discovers_repository_from_subdirectory() {
        let directory = TestDirectory::new();
        let _repository = directory.repository();
        let nested = directory.0.join("nested");
        fs::create_dir(&nested).expect("nested directory should be created");
        assert_eq!(
            GitInfo::read(&nested).expect("parent repository should be discovered").branch(),
            "main"
        );
    }

    #[test]
    fn renamed_file_uses_each_groups_current_path() {
        let directory = TestDirectory::new();
        let repository = directory.repository();
        fs::write(directory.0.join("old"), "original").expect("file should be written");
        update_index(&directory.0, Path::new("old"), IndexAction::Stage)
            .expect("file should be staged");
        let tree_id = repository
            .index()
            .expect("index should exist")
            .write_tree()
            .expect("tree should be written");
        let tree = repository.find_tree(tree_id).expect("tree should exist");
        let signature =
            Signature::now("Test", "test@example.com").expect("signature should be valid");
        repository
            .commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .expect("commit should be created");
        fs::rename(directory.0.join("old"), directory.0.join("new"))
            .expect("file should be renamed");
        update_index(&directory.0, Path::new("new"), IndexAction::Stage)
            .expect("rename should be staged");
        fs::write(directory.0.join("new"), "modified").expect("file should be modified");
        let info = GitInfo::read(&directory.0).expect("Git information should exist");
        let files = info.changed_files().expect("files should be readable");
        assert_eq!(files.len(), 1);
        let staged = files[0].change(ChangeGroup::Staged).expect("rename should be staged");
        let unstaged =
            files[0].change(ChangeGroup::Unstaged).expect("modification should be unstaged");
        assert_eq!(staged.path, Path::new("new"));
        assert_eq!(unstaged.path, Path::new("new"));
        assert_eq!(ChangedFile::status_kind(staged.status), "重命名");
        assert_eq!(ChangedFile::status_kind(unstaged.status), "修改");
        assert_eq!(staged.lines, LineChanges::Text { added: 0, deleted: 0 });
        assert_eq!(unstaged.lines, LineChanges::Text { added: 1, deleted: 1 });
    }

    #[test]
    fn reads_detached_head() {
        let directory = TestDirectory::new();
        let repository = directory.repository();
        let mut index = repository.index().expect("repository should have an index");
        let tree_id = index.write_tree().expect("empty tree should be written");
        let tree = repository.find_tree(tree_id).expect("tree should exist");
        let signature =
            Signature::now("Test", "test@example.com").expect("signature should be valid");
        let commit = repository
            .commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .expect("initial commit should be created");
        repository.set_head_detached(commit).expect("HEAD should be detached");
        let info =
            GitInfo::read(&directory.0).expect("detached repository should have Git information");
        assert_eq!(info.branch(), format!("游离 HEAD · {}", &commit.to_string()[..7]));
    }

    use std::path::Path;
}
