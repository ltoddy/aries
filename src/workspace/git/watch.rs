use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_channel::Sender;
use git2::{ErrorCode, Repository};
use gpui_kit::BackgroundExecutor;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

pub struct GitWatch {
    watcher: Option<RecommendedWatcher>,
    executor: BackgroundExecutor,
    error: Arc<Mutex<Option<String>>>,
}

impl GitWatch {
    pub fn start(
        cwd: &Path,
        sender: Sender<()>,
        executor: BackgroundExecutor,
    ) -> Result<Self, String> {
        let paths = watch_paths(cwd)?;
        let error = Arc::new(Mutex::new(None));
        let watch_error = error.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                match event {
                    Ok(event) if event.kind.is_access() => return,
                    Ok(_) => {}
                    Err(error) => {
                        *watch_error.lock().expect("Git watch error lock poisoned") =
                            Some(error.to_string());
                    }
                }
                let _ = sender.try_send(());
            })
            .map_err(|error| error.to_string())?;
        for path in paths {
            watcher
                .watch(&path, RecursiveMode::Recursive)
                .map_err(|error| format!("{}：{error}", path.display()))?;
        }
        Ok(Self { watcher: Some(watcher), executor, error })
    }

    pub fn take_error(&self) -> Option<String> {
        self.error.lock().expect("Git watch error lock poisoned").take()
    }
}

impl Drop for GitWatch {
    fn drop(&mut self) {
        if let Some(watcher) = self.watcher.take() {
            self.executor.spawn(async move { drop(watcher) }).detach();
        }
    }
}

fn watch_paths(cwd: &Path) -> Result<Vec<PathBuf>, String> {
    let repository = match Repository::discover(cwd) {
        Ok(repository) => repository,
        Err(error) if error.code() == ErrorCode::NotFound => return Ok(vec![cwd.to_owned()]),
        Err(error) => return Err(error.to_string()),
    };
    let mut paths = Vec::new();
    for path in [repository.workdir(), Some(repository.path()), Some(repository.commondir())]
        .into_iter()
        .flatten()
    {
        let path = path.canonicalize().map_err(|error| format!("{}：{error}", path.display()))?;
        if !paths.iter().any(|root: &PathBuf| path.starts_with(root)) {
            paths.retain(|root| !root.starts_with(&path));
            paths.push(path);
        }
    }
    Ok(paths)
}
