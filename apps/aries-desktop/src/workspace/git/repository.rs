use std::collections::BTreeMap;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::{Path, PathBuf};

use anyhow::Context;
use git2::{Diff, DiffFindOptions, DiffOptions, Patch, Repository, Status, StatusOptions};
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

fn read_diff(repository: &Repository, group: ChangeGroup) -> Result<Diff<'_>, git2::Error> {
    let mut options = DiffOptions::new();
    options
        .update_index(false)
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true);
    let mut diff = match group {
        ChangeGroup::Staged => {
            let tree = match repository.head() {
                Ok(head) => Some(head.peel_to_tree()?),
                Err(error) if error.code() == git2::ErrorCode::UnbornBranch => None,
                Err(error) => return Err(error),
            };
            repository.diff_tree_to_index(tree.as_ref(), None, Some(&mut options))?
        },
        ChangeGroup::Unstaged => repository.diff_index_to_workdir(None, Some(&mut options))?,
    };
    let mut find = DiffFindOptions::new();
    find.renames(true).for_untracked(true);
    diff.find_similar(Some(&mut find))?;
    Ok(diff)
}

pub fn read_file_diff(cwd: &Path, path: &Path, group: ChangeGroup) -> anyhow::Result<String> {
    let repository = Repository::discover(cwd).context("discovering Git repository")?;
    let diff = read_diff(&repository, group).context("reading Git diff")?;
    let mut output = Vec::new();
    for (ix, delta) in diff.deltas().enumerate() {
        if delta.old_file().path() != Some(path) && delta.new_file().path() != Some(path) {
            continue;
        }
        let mut patch = Patch::from_diff(&diff, ix)
            .with_context(|| format!("reading Git patch for {}", path.display()))?
            .with_context(|| format!("Git patch unavailable for {}", path.display()))?;
        let buffer = patch
            .to_buf()
            .with_context(|| format!("serializing Git patch for {}", path.display()))?;
        output.extend_from_slice(&buffer);
    }
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn read_line_changes(
    repository: &Repository,
    group: ChangeGroup,
) -> Result<BTreeMap<PathBuf, LineChanges>, git2::Error> {
    let diff = read_diff(repository, group)?;
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
            },
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Conflicted,
    Renamed,
    Deleted,
    Untracked,
    Added,
    TypeChanged,
    Modified,
}

impl StatusKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Conflicted => "冲突",
            Self::Renamed => "重命名",
            Self::Deleted => "删除",
            Self::Untracked => "未跟踪",
            Self::Added => "新增",
            Self::TypeChanged => "类型变更",
            Self::Modified => "修改",
        }
    }
}

impl Display for StatusKind {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.label())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedFile {
    path: String,
    status: Status,
    staged: Option<FileChange>,
    unstaged: Option<FileChange>,
}

impl ChangedFile {
    pub fn status_kind(status: Status) -> StatusKind {
        if status.is_conflicted() {
            StatusKind::Conflicted
        } else if status.intersects(Status::INDEX_RENAMED | Status::WT_RENAMED) {
            StatusKind::Renamed
        } else if status.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
            StatusKind::Deleted
        } else if status.is_wt_new() {
            StatusKind::Untracked
        } else if status.is_index_new() {
            StatusKind::Added
        } else if status.intersects(Status::INDEX_TYPECHANGE | Status::WT_TYPECHANGE) {
            StatusKind::TypeChanged
        } else {
            StatusKind::Modified
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
pub struct GitSummary {
    branch: String,
    file_count: Option<usize>,
}

impl Display for GitSummary {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        let branch = &self.branch;
        match self.file_count {
            Some(0) => write!(formatter, "{branch} · 工作区干净"),
            Some(count) => write!(formatter, "{branch} · {count} 个文件变更"),
            None => write!(formatter, "{branch} · 无法读取变更"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitInfo {
    branch: String,
    changed_files: Option<Vec<ChangedFile>>,
}

impl GitInfo {
    pub fn read(cwd: impl AsRef<Path>) -> anyhow::Result<Option<Self>> {
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
            },
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

    pub fn summary_state(&self) -> GitSummary {
        GitSummary {
            branch: self.branch.clone(),
            file_count: self.changed_files.as_ref().map(Vec::len),
        }
    }

    pub fn summary(&self) -> String {
        self.summary_state().to_string()
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
        match Self::status_kind(status) {
            StatusKind::Conflicted => IconName::FileExclamationPoint,
            StatusKind::Renamed => IconName::FileOutput,
            StatusKind::Deleted => IconName::FileMinus,
            StatusKind::Untracked => IconName::FileQuestionMark,
            StatusKind::Added => IconName::FilePlusCorner,
            StatusKind::TypeChanged => IconName::FileType,
            StatusKind::Modified => IconName::FilePen,
        }
    }
}
