use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;

/// Walk a directory.
///
/// - If `recursive == false`: returns direct children (files + dirs).
/// - If `recursive == true`: returns all descendants (files + dirs) in
///   breadth-first order.
pub fn walk_dir(root: impl AsRef<Path>, recursive: bool, hidden: bool) -> io::Result<Vec<PathBuf>> {
    let root = root.as_ref();

    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("Path is not a directory: {}", root.display()),
        ));
    }

    walk_dirs(&[root], recursive, hidden)
}

pub fn walk_dirs(
    roots: &[impl AsRef<Path>],
    recursive: bool,
    hidden: bool,
) -> io::Result<Vec<PathBuf>> {
    let mut queue: VecDeque<&Path> = roots.iter().map(|r| r.as_ref()).collect();
    let mut results = Vec::new();

    while let Some(dir) = queue.pop_front() {
        if !dir.is_dir() {
            continue;
        }

        let mut builder = WalkBuilder::new(dir);
        builder.hidden(hidden).ignore(true);
        if !recursive {
            builder.max_depth(Some(1));
        }

        for result in builder.build() {
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let path = entry.path();
            if path == dir {
                continue;
            }
            results.push(path.to_path_buf());
        }
    }

    Ok(results)
}
