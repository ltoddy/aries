use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct GlobalContext {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    home_dir: PathBuf,
    root_dir: PathBuf,
    memory_root_dir: PathBuf,
    worktree_dir: PathBuf,
    user: String,
    current_dir: PathBuf,
}

impl Inner {
    async fn new() -> Self {
        let home_dir = std::env::home_dir().expect("failed to determine home directory");

        let root_dir = home_dir.join(".local").join("share").join("aries");
        tokio::fs::create_dir_all(&root_dir).await.expect("failed to create Aries root directory");

        let memory_root_dir = root_dir.join("projects");
        let worktree_dir = root_dir.join("worktrees");

        let user = whoami::realname().unwrap_or_default();

        let current_dir = std::env::current_dir().expect("failed to determine current directory");

        Self { home_dir, root_dir, memory_root_dir, worktree_dir, user, current_dir }
    }
}

impl GlobalContext {
    pub async fn new() -> Self {
        Self { inner: Arc::new(Inner::new().await) }
    }

    pub fn home_dir(&self) -> &Path {
        &self.inner.home_dir
    }

    pub fn root_dir(&self) -> &Path {
        &self.inner.root_dir
    }

    pub fn memory_root_dir(&self) -> &Path {
        &self.inner.memory_root_dir
    }

    pub fn worktree_dir(&self) -> &Path {
        &self.inner.worktree_dir
    }

    pub fn user(&self) -> &str {
        &self.inner.user
    }

    pub fn current_dir(&self) -> &Path {
        &self.inner.current_dir
    }
}
