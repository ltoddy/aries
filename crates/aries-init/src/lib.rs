pub mod global;
pub mod setting;

use std::path::Path;

pub use crate::global::GlobalContext;
pub use crate::setting::{ModelConfig, Provider, Setting, SettingError, SettingLoader};

pub async fn init(root_dir: impl AsRef<Path>) {
    let mut db = aries_persistence::connect(root_dir)
        .await
        .expect("Unable to connect to local aries database");
    let _ = aries_persistence::migrate(&mut db).await;

    tokio::task::spawn(aries_persistence::gc(db));
}
