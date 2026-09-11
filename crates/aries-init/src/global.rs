use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;

#[derive(Debug, Clone)]
pub struct GlobalContext {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Debug)]
struct Inner {
    home_dir: Arc<Path>,
    root_dir: Arc<Path>,
    memory_root_dir: Arc<Path>,
    user: Arc<str>,
    current_dir: PathBuf,
}

impl GlobalContext {
    pub async fn new() -> Self {
        let home_dir = std::env::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        let root_dir = home_dir.join(".local").join("share").join("aries");
        tokio::fs::create_dir_all(&root_dir).await.expect("failed to create Aries root directory");

        let memory_root_dir = root_dir.join("projects");

        let user = whoami::realname().unwrap_or_default();

        let current_dir = std::env::current_dir().expect("failed to determine current directory");

        let inner = Inner {
            home_dir: Arc::from(home_dir),
            root_dir: Arc::from(root_dir),
            memory_root_dir: Arc::from(memory_root_dir),
            user: Arc::from(user),
            current_dir,
        };

        Self { inner: Arc::new(RwLock::new(inner)) }
    }

    pub fn home_dir(&self) -> Arc<Path> {
        Arc::clone(&self.inner.read().home_dir)
    }

    pub fn root_dir(&self) -> Arc<Path> {
        Arc::clone(&self.inner.read().root_dir)
    }

    pub fn memory_root_dir(&self) -> Arc<Path> {
        Arc::clone(&self.inner.read().memory_root_dir)
    }

    pub fn user(&self) -> Arc<str> {
        Arc::clone(&self.inner.read().user)
    }

    pub fn current_dir(&self) -> PathBuf {
        self.inner.read().current_dir.clone()
    }

    pub fn set_current_dir(&self, dir: impl Into<PathBuf>) {
        self.inner.write().current_dir = dir.into();
    }
}
