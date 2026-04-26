pub mod walk;

use std::path::{Path, PathBuf};

use url::Url;

pub use self::walk::walk_dir;

pub fn path_to_uri(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")).join(path)
    };

    Url::from_file_path(&absolute_path)
        .map(|u| u.to_string())
        .unwrap_or_else(|_| format!("file://{}", absolute_path.display()))
}
