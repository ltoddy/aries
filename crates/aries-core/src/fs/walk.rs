use std::io;
use std::path::PathBuf;

use ignore::WalkBuilder;

/// Walk a directory.
///
/// - If `recursive == false`: returns direct children (files + dirs).
/// - If `recursive == true`: returns all descendants (files + dirs) in
///   breadth-first order.
pub async fn walk_dir(root: PathBuf, recursive: bool, hidden: bool) -> io::Result<Vec<PathBuf>> {
    if !root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("Path is not a directory: {}", root.display()),
        ));
    }

    let mut builder = WalkBuilder::new(&root);
    builder.hidden(hidden).ignore(true);
    if !recursive {
        builder.max_depth(Some(1));
    }

    tokio::task::spawn_blocking(move || {
        let mut results = Vec::new();
        for result in builder.build() {
            let entry = match result {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let path = entry.path();
            if path == root {
                continue;
            }
            results.push(path.to_path_buf());
        }

        results
    })
    .await
    .map_err(|err| io::Error::other(err.to_string()))
}
