use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_channel::Sender;
use git2::{ErrorCode, Repository, Status};
use gpui_kit::BackgroundExecutor;
use notify::event::{CreateKind, RemoveKind};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::workspace::git::error::WatchError;

pub struct GitWatch {
    watcher: Option<RecommendedWatcher>,
    executor: BackgroundExecutor,
    error: Arc<Mutex<Option<notify::Error>>>,
    paths: Arc<Mutex<Vec<PathBuf>>>,
}

impl GitWatch {
    pub fn start(
        cwd: impl AsRef<Path>,
        sender: Sender<()>,
        executor: BackgroundExecutor,
    ) -> Result<Self, WatchError> {
        let (paths, repository_path) = watch_paths(cwd.as_ref())?;
        Self::start_filtered(paths, sender, executor, move |event| {
            should_refresh(event, repository_path.as_deref())
        })
    }

    pub fn start_tree(
        cwd: &Path,
        sender: Sender<()>,
        executor: BackgroundExecutor,
    ) -> Result<Self, WatchError> {
        let (paths, _) = watch_paths(cwd)?;
        Self::start_filtered(paths, sender, executor, |_| true)
    }

    pub fn start_file(
        directory: &Path,
        path: &Path,
        sender: Sender<()>,
        executor: BackgroundExecutor,
    ) -> Result<(Self, Option<PathBuf>), WatchError> {
        let path = std::path::absolute(path).map_err(|source| WatchError::resolve(path, source))?;
        let directory = std::path::absolute(directory)
            .map_err(|source| WatchError::resolve(directory, source))?;
        let mut targets = vec![path.clone()];
        let mut paths = Vec::new();
        let root = if path.starts_with(&directory) {
            directory.as_path()
        } else {
            path.parent().unwrap_or(&path)
        };
        insert_watch_path(&mut paths, existing_watch_root(root)?);
        let identity = Self::file_identity(&path);
        if let Some(resolved) = identity.as_ref() {
            insert_watch_path(
                &mut paths,
                existing_watch_root(resolved.parent().unwrap_or(resolved))?,
            );
            targets.push(resolved.clone());
        }
        if let Ok(resolved) = root.canonicalize()
            && let Ok(relative) = path.strip_prefix(root)
        {
            targets.push(resolved.join(relative));
        }
        let watcher = Self::start_filtered(paths, sender, executor, move |event| {
            event.need_rescan()
                || event.paths.is_empty()
                || event
                    .paths
                    .iter()
                    .any(|changed| targets.iter().any(|target| target.starts_with(changed)))
        })?;
        Ok((watcher, identity))
    }

    pub fn file_identity(path: &Path) -> Option<PathBuf> {
        let mut path = std::path::absolute(path).ok()?;
        for _ in 0..40 {
            if let Ok(resolved) = path.canonicalize() {
                return Some(resolved);
            }
            let mut link = None;
            for ancestor in path.ancestors() {
                if let Ok(target) = std::fs::read_link(ancestor) {
                    let target =
                        if target.is_absolute() { target } else { ancestor.parent()?.join(target) };
                    link = Some(target.join(path.strip_prefix(ancestor).ok()?));
                    break;
                }
                if let Ok(resolved) = ancestor.canonicalize() {
                    return Some(resolved.join(path.strip_prefix(ancestor).ok()?));
                }
            }
            path = link?;
        }
        None
    }

    fn start_filtered(
        paths: Vec<PathBuf>,
        sender: Sender<()>,
        executor: BackgroundExecutor,
        should_refresh: impl Fn(&notify::Event) -> bool + Send + 'static,
    ) -> Result<Self, WatchError> {
        let error = Arc::new(Mutex::new(None));
        let watch_error = error.clone();
        let touched = Arc::new(Mutex::new(Vec::new()));
        let changed_paths = touched.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                match event {
                    Ok(event) => {
                        if event.kind.is_access() || !should_refresh(&event) {
                            return;
                        }
                        if let Ok(mut paths) = changed_paths.lock() {
                            paths.extend(event.paths);
                        }
                    },
                    Err(error) => {
                        if let Ok(mut watch_error) = watch_error.lock() {
                            *watch_error = Some(error);
                        }
                    },
                }
                let _ = sender.try_send(());
            })
            .map_err(WatchError::create)?;
        for path in paths {
            watcher
                .watch(&path, RecursiveMode::Recursive)
                .map_err(|source| WatchError::watch(&path, source))?;
        }
        Ok(Self { watcher: Some(watcher), executor, error, paths: touched })
    }

    pub fn take_error(&self) -> Result<Option<WatchError>, WatchError> {
        let mut error = self.error.lock().map_err(|_| WatchError::lock_poisoned())?;
        Ok(error.take().map(WatchError::runtime))
    }

    pub fn take_paths(&self) -> Vec<PathBuf> {
        self.paths.lock().map(|mut paths| std::mem::take(&mut *paths)).unwrap_or_default()
    }
}

impl Drop for GitWatch {
    fn drop(&mut self) {
        if let Some(watcher) = self.watcher.take() {
            self.executor.spawn(async move { drop(watcher) }).detach();
        }
    }
}

fn should_refresh(event: &notify::Event, repository_path: Option<&Path>) -> bool {
    if event.need_rescan() || event.paths.is_empty() {
        return true;
    }
    let Some(repository_path) = repository_path else {
        return true;
    };
    let Ok(repository) = Repository::open(repository_path) else {
        return true;
    };
    let Some(workdir) = repository.workdir() else {
        return true;
    };
    event.paths.iter().any(|path| {
        if path.starts_with(repository.path())
            || path.starts_with(repository.commondir())
            || path.file_name().is_some_and(|name| name == ".gitignore" || name == ".git")
        {
            return true;
        }
        let Ok(relative_path) = path.strip_prefix(workdir) else {
            return true;
        };
        if repository.is_path_ignored(relative_path) != Ok(true)
            && (!matches!(
                event.kind,
                EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder)
            ) || repository.is_path_ignored(relative_path.join("")) != Ok(true))
        {
            return true;
        }
        match repository.status_file(relative_path) {
            Ok(status) => !status.intersects(Status::IGNORED | Status::WT_NEW),
            Err(error) if error.code() == ErrorCode::Ambiguous => {
                let Ok(index) = repository.index() else {
                    return true;
                };
                index.iter().any(|entry| {
                    std::str::from_utf8(&entry.path)
                        .map(|path| Path::new(path).starts_with(relative_path))
                        .unwrap_or(true)
                })
            },
            Err(error) => error.code() != ErrorCode::NotFound,
        }
    })
}

fn watch_paths(cwd: &Path) -> Result<(Vec<PathBuf>, Option<PathBuf>), WatchError> {
    let repository = match Repository::discover(cwd) {
        Ok(repository) => repository,
        Err(error) if error.code() == ErrorCode::NotFound => {
            return Ok((vec![cwd.to_owned()], None));
        },
        Err(error) => return Err(WatchError::discover(cwd, error)),
    };
    let mut paths = Vec::new();
    for path in [repository.workdir(), Some(repository.path()), Some(repository.commondir())]
        .into_iter()
        .flatten()
    {
        let path = path.canonicalize().map_err(|source| WatchError::resolve(path, source))?;
        insert_watch_path(&mut paths, path);
    }
    Ok((paths, Some(repository.path().to_owned())))
}

fn insert_watch_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|root| path.starts_with(root)) {
        paths.retain(|root| !root.starts_with(&path));
        paths.push(path);
    }
}

fn existing_watch_root(path: &Path) -> Result<PathBuf, WatchError> {
    for ancestor in path.ancestors() {
        match ancestor.canonicalize() {
            Ok(resolved) if resolved.is_dir() => return Ok(resolved),
            Ok(_) => {},
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(WatchError::resolve(ancestor, error)),
        }
    }
    Err(WatchError::resolve(
        path,
        std::io::Error::new(std::io::ErrorKind::NotFound, "监听目录不存在"),
    ))
}
