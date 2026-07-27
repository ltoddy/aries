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

    let lock_file_path = root_dir.join("aries-db.lock");
    match aries_filesystem::lock::try_lock(lock_file_path).await {
        Ok(file) => {
            black_box(file); // 避免优化器优化掉 file 导致 file drop 了 (可能不会出现这个情况)
            aries_persistence::gc(db).await;
        },
        Err(err) => warn!(err = %err, "failed to lock file"),
    }
}
