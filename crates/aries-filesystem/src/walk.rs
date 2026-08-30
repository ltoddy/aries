use std::io;
use std::path::{Path, PathBuf};

use ignore::{DirEntry, Error, WalkBuilder, WalkState};
use parking_lot::Mutex;

/// Walk a directory.
///
/// - If `recursive == false`: returns direct children (files + dirs).
/// - If `recursive == true`: returns all descendants (files + dirs).
/// - `hidden`: if `true`, hidden entries (dotfiles) are ignored.
pub fn walk_dir(root: impl AsRef<Path>, recursive: bool, hidden: bool) -> io::Result<Vec<PathBuf>> {
    let root = root.as_ref();

    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("Path is not a directory: {}", root.display()),
        ));
    }

    walk_dirs([root], recursive, hidden)
}

pub fn walk_dirs(
    roots: impl IntoIterator<Item = impl AsRef<Path>>,
    recursive: bool,
    hidden: bool,
) -> io::Result<Vec<PathBuf>> {
    let mut entries = Vec::new();

    for root in roots {
        let root = root.as_ref();
        if !root.is_dir() {
            continue;
        }

        let mut builder = WalkBuilder::new(root);
        builder.hidden(hidden).ignore(true);
        if !recursive {
            builder.max_depth(Some(1));
        }

        let visited = Mutex::new(Vec::new());
        builder.build_parallel().run(|| {
            Box::new(|result: Result<DirEntry, Error>| {
                let entry = match result {
                    Ok(entry) => entry,
                    Err(_) => return WalkState::Continue,
                };

                let path = entry.path();
                if path == root {
                    return WalkState::Continue;
                }
                visited.lock().push(path.to_owned());
                WalkState::Continue
            })
        });
        entries.append(&mut visited.into_inner());
    }

    Ok(entries)
}
