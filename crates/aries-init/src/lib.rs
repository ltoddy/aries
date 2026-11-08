pub mod agents;
pub mod global;
pub mod setting;

use std::hint::black_box;
use std::path::Path;

use tracing::warn;

pub use crate::global::GlobalContext;
pub use crate::setting::{ModelConfig, Provider, Setting, SettingError, SettingLoader};

pub async fn init(root_dir: impl AsRef<Path>) {
    let root_dir = root_dir.as_ref();

    let mut db = aries_persistence::connect(root_dir)
        .await
        .expect("Unable to connect to local aries database");
    let _ = aries_persistence::migrate(&mut db).await;

    const GENERAL: &str = include_str!("agents/general.md");
    const EXPLORE: &str = include_str!("agents/explore.md");
    const BUILD: &str = include_str!("agents/build.md");

    let agents_dir = root_dir.join("agents");
    tokio::fs::create_dir_all(&agents_dir).await.expect("Unable to create agents directory");

    let (f1, f2, f3) = tokio::join!(
        tokio::fs::write(agents_dir.join("build.md"), BUILD),
        tokio::fs::write(agents_dir.join("explore.md"), EXPLORE),
        tokio::fs::write(agents_dir.join("general.md"), GENERAL),
    );
    f1.expect("Unable to write build.md");
    f2.expect("Unable to write explore.md");
    f3.expect("Unable to write general.md");
}

pub async fn gc(root_dir: impl AsRef<Path>) {
    let root_dir = root_dir.as_ref();

    let mut db = aries_persistence::connect(root_dir)
        .await
        .expect("Unable to connect to local aries database");
    let _ = aries_persistence::migrate(&mut db).await;

    let lock_file_path = root_dir.join("aries-db.lock");
    match aries_filesystem::lock::lock(lock_file_path).await {
        Ok(file) => {
            black_box(file); // 避免优化器优化掉 file 导致 file drop 了 (可能不会出现这个情况)
            aries_persistence::gc(db).await;
        },
        Err(err) => warn!(err = %err, "failed to lock file"),
    }
}
