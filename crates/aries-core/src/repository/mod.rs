use std::io;
use std::path::Path;

use git2::Repository;

use crate::fs::walk_dir;

pub fn count_files(root: impl AsRef<Path>) -> io::Result<usize> {
    let root = root.as_ref();

    let _ = Repository::discover(root)
        .map_err(|err| io::Error::new(io::ErrorKind::Unsupported, err))?;

    let entries = walk_dir(root, true, true)?;
    let file_paths = entries.into_iter().filter(|e| e.is_file()).collect::<Vec<_>>();

    let files = file_paths.len();

    Ok(files)
}
