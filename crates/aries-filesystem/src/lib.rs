pub mod jsonl;
pub mod walk;

use std::io;
use std::path::{Path, PathBuf};

use git2::Repository;
use url::Url;

use crate::walk::walk_dir;

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

pub fn count_files(root: impl AsRef<Path>) -> io::Result<usize> {
    let root = root.as_ref();

    let _ = Repository::discover(root)
        .map_err(|err| io::Error::new(io::ErrorKind::Unsupported, err))?;

    let entries = walk_dir(root, true, true)?;
    let file_paths = entries.into_iter().filter(|e| e.is_file()).collect::<Vec<_>>();

    let files = file_paths.len();

    Ok(files)
}
