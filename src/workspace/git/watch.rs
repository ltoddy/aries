use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Context;
use async_channel::Sender;
use git2::{ErrorCode, Repository};
use gpui_kit::BackgroundExecutor;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

pub struct GitWatch {
    watcher: Option<RecommendedWatcher>,
    executor: BackgroundExecutor,
    error: Arc<Mutex<Option<notify::Error>>>,
}

impl GitWatch {
    pub fn start(
        cwd: &Path,
        sender: Sender<()>,
        executor: BackgroundExecutor,
    ) -> anyhow::Result<Self> {
        let paths = watch_paths(cwd)?;
        let error = Arc::new(Mutex::new(None));
        let watch_error = error.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                match event {
                    Ok(event) if event.kind.is_access() => return,
                    Ok(_) => {}
                    Err(error) => {
                        *watch_error.lock().expect("Git watch error lock poisoned") = Some(error);
                    }
                }
                let _ = sender.try_send(());
            })
            .context("creating Git watcher")?;
        for path in paths {
            watcher
                .watch(&path, RecursiveMode::Recursive)
                .with_context(|| format!("watching {}", path.display()))?;
        }
        Ok(Self { watcher: Some(watcher), executor, error })
    }

    pub fn take_error(&self) -> Option<notify::Error> {
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

fn watch_paths(cwd: &Path) -> anyhow::Result<Vec<PathBuf>> {
    let repository = match Repository::discover(cwd) {
        Ok(repository) => repository,
        Err(error) if error.code() == ErrorCode::NotFound => return Ok(vec![cwd.to_owned()]),
        Err(error) => return Err(error).context("discovering Git watch repository"),
    };
    let mut paths = Vec::new();
    for path in [repository.workdir(), Some(repository.path()), Some(repository.commondir())]
        .into_iter()
        .flatten()
    {
        let path = path
            .canonicalize()
            .with_context(|| format!("resolving Git watch path {}", path.display()))?;
        if !paths.iter().any(|root: &PathBuf| path.starts_with(root)) {
            paths.retain(|root| !root.starts_with(&path));
            paths.push(path);
        }
    }
    Ok(paths)
}
