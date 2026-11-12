use std::path::{Path, PathBuf};

use gpui_kit::component::diff::DiffParseError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RepositoryError {
    #[error("{operation} at {}", path.display())]
    Git { operation: &'static str, path: PathBuf, source: git2::Error },
    #[error("Git patch unavailable for {}", path.display())]
    PatchUnavailable { path: PathBuf },
    #[error("HEAD has no target")]
    HeadTargetMissing,
}

impl RepositoryError {
    pub fn git(operation: &'static str, path: &Path, source: git2::Error) -> Self {
        Self::Git { operation, path: path.to_owned(), source }
    }

    pub fn patch_unavailable(path: &Path) -> Self {
        Self::PatchUnavailable { path: path.to_owned() }
    }

    pub fn head_target_missing() -> Self {
        Self::HeadTargetMissing
    }
}

#[derive(Debug, Error)]
pub enum WatchError {
    #[error("discovering Git watch repository at {}", path.display())]
    Discover { path: PathBuf, source: git2::Error },
    #[error("resolving Git watch path {}", path.display())]
    Resolve { path: PathBuf, source: std::io::Error },
    #[error("creating Git watcher")]
    Create { source: notify::Error },
    #[error("Git 自动刷新失败，请手动刷新重试")]
    Runtime { source: notify::Error },
    #[error("Git 自动刷新状态锁已损坏，请手动刷新以重新启动监听")]
    LockPoisoned,
    #[error("watching {}", path.display())]
    Watch { path: PathBuf, source: notify::Error },
}

impl WatchError {
    pub fn runtime(source: notify::Error) -> Self {
        Self::Runtime { source }
    }

    pub fn lock_poisoned() -> Self {
        Self::LockPoisoned
    }

    pub fn discover(path: &Path, source: git2::Error) -> Self {
        Self::Discover { path: path.to_owned(), source }
    }

    pub fn resolve(path: &Path, source: std::io::Error) -> Self {
        Self::Resolve { path: path.to_owned(), source }
    }

    pub fn create(source: notify::Error) -> Self {
        Self::Create { source }
    }

    pub fn watch(path: &Path, source: notify::Error) -> Self {
        Self::Watch { path: path.to_owned(), source }
    }
}

#[derive(Debug, Error)]
pub enum DiffLoadError {
    #[error(transparent)]
    Repository { source: RepositoryError },
    #[error("parsing display diff for {}", path.display())]
    DisplayPatch { path: PathBuf, source: DiffParseError },
}

impl DiffLoadError {
    pub fn display_patch(path: &Path, source: DiffParseError) -> Self {
        Self::DisplayPatch { path: path.to_owned(), source }
    }
}

impl From<RepositoryError> for DiffLoadError {
    fn from(source: RepositoryError) -> Self {
        Self::Repository { source }
    }
}

#[derive(Debug, Error)]
pub enum GitIoError {
    #[error("{operation} at {}", path.display())]
    File { operation: &'static str, path: PathBuf, source: std::io::Error },
}

impl GitIoError {
    pub fn new(operation: &'static str, path: &Path, source: std::io::Error) -> Self {
        Self::File { operation, path: path.to_owned(), source }
    }
}
