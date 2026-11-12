use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_channel::Sender;
use git2::{ErrorCode, Repository, Status};
use gpui_kit::BackgroundExecutor;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::workspace::git::error::WatchError;

pub struct GitWatch {
    watcher: Option<RecommendedWatcher>,
    executor: BackgroundExecutor,
    error: Arc<Mutex<Option<notify::Error>>>,
}

impl GitWatch {
    pub fn start(
        cwd: impl AsRef<Path>,
        sender: Sender<()>,
        executor: BackgroundExecutor,
    ) -> Result<Self, WatchError> {
        let (paths, repository_path) = watch_paths(cwd.as_ref())?;
        let error = Arc::new(Mutex::new(None));
        let watch_error = error.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                match event {
                    Ok(event) => {
                        if event.kind.is_access()
                            || !should_refresh(&event, repository_path.as_deref())
                        {
                            return;
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
        Ok(Self { watcher: Some(watcher), executor, error })
    }

    pub fn take_error(&self) -> Result<Option<WatchError>, WatchError> {
        let mut error = self.error.lock().map_err(|_| WatchError::lock_poisoned())?;
        Ok(error.take().map(WatchError::runtime))
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
        if repository.is_path_ignored(relative_path) != Ok(true) {
            return true;
        }
        match repository.status_file(relative_path) {
            Ok(status) => !status.intersects(Status::IGNORED | Status::WT_NEW),
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
        if !paths.iter().any(|root: &PathBuf| path.starts_with(root)) {
            paths.retain(|root| !root.starts_with(&path));
            paths.push(path);
        }
    }
    Ok((paths, Some(repository.path().to_owned())))
}
