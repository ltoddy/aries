use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use git2::{DiffFindOptions, DiffOptions, Patch, Repository, Status, StatusOptions};
use gpui_kit::assets::IconName;
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;

use crate::workspace::git::index::IndexAction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeGroup {
    Staged,
    Unstaged,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    path: PathBuf,
    status: Status,
    lines: LineChanges,
}

impl FileChange {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn status(&self) -> Status {
        self.status
    }

    pub fn lines(&self) -> LineChanges {
        self.lines
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineChanges {
    Text { added: usize, deleted: usize },
    Binary,
    Unavailable,
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
    pub fn status_kind(status: Status) -> &'static str {
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

    pub fn change(&self, group: ChangeGroup) -> Option<&FileChange> {
        match group {
            ChangeGroup::Staged => self.staged.as_ref(),
            ChangeGroup::Unstaged => self.unstaged.as_ref(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitInfo {
    branch: String,
    changed_files: Option<Vec<ChangedFile>>,
}

impl GitInfo {
    pub fn read(cwd: &Path) -> anyhow::Result<Option<Self>> {
        let repository = match Repository::discover(cwd) {
            Ok(repository) => repository,
            Err(error) if error.code() == git2::ErrorCode::NotFound => return Ok(None),
            Err(error) => return Err(error).context("discovering Git repository"),
        };
        let head = repository.find_reference("HEAD").context("reading HEAD reference")?;
        let branch = match head.symbolic_target().context("reading HEAD symbolic target")? {
            Some(target) => target.strip_prefix("refs/heads/").unwrap_or(target).to_owned(),
            None => {
                format!("游离 HEAD · {}", &head.target().context("HEAD 没有目标")?.to_string()[..7])
            }
        };
        let mut options = StatusOptions::new();
        options
            .update_index(false)
            .include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);
        let staged_lines = read_line_changes(&repository, ChangeGroup::Staged).ok();
        let unstaged_lines = read_line_changes(&repository, ChangeGroup::Unstaged).ok();
        let changed_files = Some({
            let statuses = repository.statuses(Some(&mut options)).context("reading Git status")?;
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

impl ChangeGroup {
    pub fn id(self) -> &'static str {
        match self {
            Self::Staged => "git-staged",
            Self::Unstaged => "git-unstaged",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Staged => "已暂存",
            Self::Unstaged => "未暂存",
        }
    }

    pub fn action(self) -> IndexAction {
        match self {
            Self::Staged => IndexAction::Unstage,
            Self::Unstaged => IndexAction::Stage,
        }
    }
}

impl LineChanges {
    pub fn render(self, cx: &App) -> AnyElement {
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

impl ChangedFile {
    pub fn status_icon(status: Status) -> IconName {
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
}
