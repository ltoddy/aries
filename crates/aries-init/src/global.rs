use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct GlobalContext {
    pub home_dir: PathBuf,
    pub root_dir: PathBuf,
    pub memory_dir: PathBuf,
    pub user: String,
}

impl GlobalContext {
    pub async fn new() -> Self {
        let home_dir = std::env::home_dir().unwrap_or_else(|| PathBuf::from("~"));

        let root_dir = home_dir.join(".local").join("share").join("aries");
        tokio::fs::create_dir_all(&root_dir).await.expect("failed to create Aries root directory");

        let memory_dir = root_dir.join("memory");

        let user = whoami::realname().unwrap_or_default();

        Self { home_dir, root_dir, memory_dir, user }
    }
}
