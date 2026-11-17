use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::{fs, io};

use git2::{ErrorCode, Repository, Status, StatusOptions};
use gpui_kit::component::ActiveTheme;
use gpui_kit::{App, Hsla};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryGitStatus {
    Conflicted,
    Renamed,
    Deleted,
    TypeChanged,
    Modified,
    Added,
    Untracked,
    Ignored,
}

impl EntryGitStatus {
    pub fn conflicted() -> Self {
        Self::Conflicted
    }

    pub fn renamed() -> Self {
        Self::Renamed
    }

    pub fn deleted() -> Self {
        Self::Deleted
    }

    pub fn type_changed() -> Self {
        Self::TypeChanged
    }

    pub fn modified() -> Self {
        Self::Modified
    }

    pub fn added() -> Self {
        Self::Added
    }

    pub fn untracked() -> Self {
        Self::Untracked
    }

    pub fn ignored() -> Self {
        Self::Ignored
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Conflicted => "冲突",
            Self::Renamed => "重命名",
            Self::Deleted => "删除",
            Self::TypeChanged => "类型变更",
            Self::Modified => "修改",
            Self::Added => "新增",
            Self::Untracked => "未跟踪",
            Self::Ignored => "已忽略",
        }
    }

    pub fn marker(&self) -> &'static str {
        match self {
            Self::Conflicted => "!",
            Self::Renamed => "R",
            Self::Deleted => "D",
            Self::TypeChanged | Self::Modified => "M",
            Self::Added => "A",
            Self::Untracked => "?",
            Self::Ignored => "I",
        }
    }

    pub fn color(&self, cx: &App) -> Hsla {
        match self {
            Self::Conflicted | Self::Deleted => cx.theme().danger,
            Self::Renamed | Self::TypeChanged | Self::Modified => cx.theme().info,
            Self::Added => cx.theme().success,
            Self::Untracked => cx.theme().warning,
            Self::Ignored => cx.theme().muted_foreground,
        }
    }

    fn from_status(status: Status) -> Option<Self> {
        if status.is_conflicted() {
            Some(Self::conflicted())
        } else if status.intersects(Status::INDEX_RENAMED | Status::WT_RENAMED) {
            Some(Self::renamed())
        } else if status.intersects(Status::INDEX_DELETED | Status::WT_DELETED) {
            Some(Self::deleted())
        } else if status.intersects(Status::INDEX_TYPECHANGE | Status::WT_TYPECHANGE) {
            Some(Self::type_changed())
        } else if status.intersects(Status::INDEX_MODIFIED | Status::WT_MODIFIED) {
            Some(Self::modified())
        } else if status.is_index_new() {
            Some(Self::added())
        } else if status.is_wt_new() {
            Some(Self::untracked())
        } else if status.is_ignored() {
            Some(Self::ignored())
        } else {
            None
        }
    }

    fn priority(&self) -> u8 {
        match self {
            Self::Conflicted => 8,
            Self::Renamed => 7,
            Self::Deleted => 6,
            Self::TypeChanged => 5,
            Self::Modified => 4,
            Self::Added => 3,
            Self::Untracked => 2,
            Self::Ignored => 1,
        }
    }
}

#[derive(Debug, Error)]
pub enum ReadTreeGitStatusError {
    #[error("resolving Git status path {}: {source}", path.display())]
    Resolve { path: PathBuf, source: io::Error },
    #[error("{operation} at {}: {source}", path.display())]
    Git { operation: &'static str, path: PathBuf, source: git2::Error },
}

impl ReadTreeGitStatusError {
    pub fn resolve(path: &Path, source: io::Error) -> Self {
        Self::Resolve { path: path.to_owned(), source }
    }

    pub fn git(operation: &'static str, path: &Path, source: git2::Error) -> Self {
        Self::Git { operation, path: path.to_owned(), source }
    }
}

#[derive(Clone, Debug)]
pub struct TreeGitStatus {
    root: Option<PathBuf>,
    resolved_root: Option<PathBuf>,
    entries: HashMap<PathBuf, EntryGitStatus>,
    tracked_paths: HashSet<PathBuf>,
    tracked_directories: HashSet<PathBuf>,
    ignored_directories: HashSet<PathBuf>,
}

impl Default for TreeGitStatus {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeGitStatus {
    pub fn new() -> Self {
        Self {
            root: None,
            resolved_root: None,
            entries: HashMap::new(),
            tracked_paths: HashSet::new(),
            tracked_directories: HashSet::new(),
            ignored_directories: HashSet::new(),
        }
    }

    pub fn read(root: &Path) -> Result<Self, ReadTreeGitStatusError> {
        let resolved_root = fs::canonicalize(root)
            .map_err(|source| ReadTreeGitStatusError::resolve(root, source))?;
        let repository = match Repository::discover(&resolved_root) {
            Ok(repository) => repository,
            Err(error) if error.code() == ErrorCode::NotFound => return Ok(Self::new()),
            Err(error) => {
                return Err(ReadTreeGitStatusError::git("discovering Git repository", root, error));
            },
        };
        let Some(workdir) = repository.workdir() else {
            return Ok(Self::new());
        };
        let workdir = fs::canonicalize(workdir)
            .map_err(|source| ReadTreeGitStatusError::resolve(workdir, source))?;
        let Ok(project_path) = resolved_root.strip_prefix(&workdir) else {
            return Ok(Self::new());
        };
        if !is_tree_path(project_path) {
            return Ok(Self::new());
        }
        let provided_root = std::path::absolute(root)
            .map_err(|source| ReadTreeGitStatusError::resolve(root, source))?;
        let index = repository
            .index()
            .map_err(|source| ReadTreeGitStatusError::git("reading Git index", root, source))?;
        let mut tracked_paths = HashSet::new();
        let mut tracked_directories = HashSet::new();
        let mut directories: HashSet<PathBuf> =
            project_path.ancestors().map(Path::to_owned).collect();
        for entry in index.iter() {
            let relative = path_from_bytes(&entry.path);
            if !is_tree_path(&relative) {
                continue;
            }
            let repository_path = workdir.join(&relative);
            let Ok(tree_relative) = repository_path.strip_prefix(&resolved_root) else {
                continue;
            };
            let path = provided_root.join(tree_relative);
            for ancestor in
                path.ancestors().skip(1).take_while(|ancestor| ancestor.starts_with(&provided_root))
            {
                if !tracked_directories.insert(ancestor.to_owned()) {
                    break;
                }
            }
            tracked_paths.insert(path);
            for ancestor in relative.ancestors().skip(1) {
                if !directories.insert(ancestor.to_owned()) {
                    break;
                }
            }
        }
        let mut ignored_directories = HashSet::new();
        for mut directory in directories {
            if directory.as_os_str().is_empty() {
                continue;
            }
            directory.as_mut_os_string().push("/");
            let repository_path = workdir.join(&directory);
            if !repository.status_should_ignore(&directory).map_err(|source| {
                ReadTreeGitStatusError::git("checking Git ignore rules", &repository_path, source)
            })? {
                continue;
            }
            let path = match repository_path.strip_prefix(&resolved_root) {
                Ok(relative) => provided_root.join(relative),
                Err(_) if resolved_root.starts_with(&repository_path) => provided_root.clone(),
                Err(_) => continue,
            };
            ignored_directories.insert(path);
        }
        let mut tree = Self::with_roots(provided_root, resolved_root);
        tree.tracked_paths = tracked_paths;
        tree.tracked_directories = tracked_directories;
        tree.ignored_directories = ignored_directories;
        let mut options = StatusOptions::new();
        options
            .update_index(false)
            .include_untracked(true)
            .include_ignored(true)
            .recurse_untracked_dirs(true)
            .recurse_ignored_dirs(false)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true);
        let statuses = repository
            .statuses(Some(&mut options))
            .map_err(|source| ReadTreeGitStatusError::git("reading Git status", root, source))?;
        for entry in statuses.iter() {
            let Some(status) = EntryGitStatus::from_status(entry.status()) else {
                continue;
            };
            let head_to_index = entry.head_to_index();
            let index_to_workdir = entry.index_to_workdir();
            let paths = [
                Some(entry.path_bytes()),
                head_to_index.as_ref().and_then(|delta| delta.old_file().path_bytes()),
                head_to_index.as_ref().and_then(|delta| delta.new_file().path_bytes()),
                index_to_workdir.as_ref().and_then(|delta| delta.old_file().path_bytes()),
                index_to_workdir.as_ref().and_then(|delta| delta.new_file().path_bytes()),
            ];
            for (ix, bytes) in paths.iter().enumerate() {
                let Some(bytes) = bytes else {
                    continue;
                };
                if !paths[..ix].contains(&Some(*bytes)) {
                    tree.insert_repository_path(&workdir, bytes, status);
                }
            }
        }
        Ok(tree)
    }

    pub fn status(&self, path: &Path) -> Option<EntryGitStatus> {
        let root = self.root.as_deref()?;
        let resolved_root = self.resolved_root.as_deref()?;
        let path = std::path::absolute(path).ok()?;
        let relative =
            path.strip_prefix(root).or_else(|_| path.strip_prefix(resolved_root)).ok()?;
        if !is_tree_path(relative) {
            return None;
        }
        let path = root.join(relative);
        let status = self.entries.get(&path).copied();
        if status.is_some_and(|status| status != EntryGitStatus::ignored()) {
            return status;
        }
        if self.tracked_paths.contains(&path) || self.tracked_directories.contains(&path) {
            return None;
        }
        status.or_else(|| {
            path.ancestors()
                .take_while(|ancestor| ancestor.starts_with(root))
                .find(|ancestor| self.ignored_directories.contains(*ancestor))
                .map(|_| EntryGitStatus::ignored())
        })
    }

    fn with_roots(root: PathBuf, resolved_root: PathBuf) -> Self {
        let mut tree = Self::new();
        tree.root = Some(root);
        tree.resolved_root = Some(resolved_root);
        tree
    }

    fn insert_repository_path(&mut self, workdir: &Path, bytes: &[u8], status: EntryGitStatus) {
        let relative = path_from_bytes(bytes);
        if !is_tree_path(&relative) {
            return;
        }
        let Some(root) = self.root.as_deref() else {
            return;
        };
        let Some(resolved_root) = self.resolved_root.as_deref() else {
            return;
        };
        let repository_path = workdir.join(relative);
        let ignored_directory = status == EntryGitStatus::ignored() && bytes.ends_with(b"/");
        let path = match repository_path.strip_prefix(resolved_root) {
            Ok(relative) => root.join(relative),
            Err(_) if ignored_directory && resolved_root.starts_with(&repository_path) => {
                root.to_owned()
            },
            Err(_) => return,
        };
        if ignored_directory {
            self.ignored_directories.insert(path.clone());
        }
        for ancestor in path.ancestors().take_while(|ancestor| ancestor.starts_with(root)) {
            self.entries
                .entry(ancestor.to_owned())
                .and_modify(|current| {
                    if status.priority() > current.priority() {
                        *current = status;
                    }
                })
                .or_insert(status);
        }
    }
}

fn is_tree_path(path: &Path) -> bool {
    path.components().all(|component| match component {
        Component::Normal(name) => name != ".git",
        Component::CurDir => true,
        _ => false,
    })
}

#[cfg(unix)]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    PathBuf::from(OsStr::from_bytes(bytes))
}

#[cfg(not(unix))]
fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    PathBuf::from(String::from_utf8_lossy(bytes).as_ref())
}
