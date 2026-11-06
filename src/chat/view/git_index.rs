use std::fs;
use std::path::{Component, Path};

use git2::{Error, ErrorCode, Index, Repository, Status, StatusOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IndexAction {
    Stage,
    Unstage,
}

pub(super) fn update_index(cwd: &Path, path: &Path, action: IndexAction) -> Result<(), Error> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| !matches!(component, Component::Normal(_)))
        || path.as_os_str().as_encoded_bytes().ends_with(b"/")
    {
        return Err(Error::from_str("expected a repository-relative file path"));
    }

    let repository = Repository::discover(cwd)?;
    let workdir =
        repository.workdir().ok_or_else(|| Error::from_str("repository has no workdir"))?;
    match fs::symlink_metadata(workdir.join(path)) {
        Ok(metadata) if metadata.is_dir() => {
            return Err(Error::from_str("directories cannot be staged or unstaged"));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(Error::from_str(&error.to_string())),
    }

    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repository.statuses(Some(&mut options))?;
    let mut matches = statuses.iter().filter(|entry| {
        [entry.head_to_index(), entry.index_to_workdir()].into_iter().flatten().any(|delta| {
            delta.old_file().path() == Some(path) || delta.new_file().path() == Some(path)
        })
    });
    let entry = matches.next().ok_or_else(|| Error::from_str("file has no current changes"))?;
    if matches.next().is_some() {
        return Err(Error::from_str("file has ambiguous rename status"));
    }
    let status = entry.status();
    if status.is_conflicted() {
        return Err(Error::from_str("conflicted files cannot be staged or unstaged"));
    }

    let mut paths = vec![path.to_path_buf()];
    let required = match action {
        IndexAction::Stage => {
            if status.is_wt_renamed()
                && let Some(delta) = entry.index_to_workdir()
            {
                paths = [delta.old_file().path(), delta.new_file().path()]
                    .into_iter()
                    .flatten()
                    .map(Path::to_path_buf)
                    .collect();
            }
            Status::WT_NEW
                | Status::WT_MODIFIED
                | Status::WT_DELETED
                | Status::WT_RENAMED
                | Status::WT_TYPECHANGE
        }
        IndexAction::Unstage => {
            if status.is_index_renamed()
                && let Some(delta) = entry.head_to_index()
            {
                paths = [delta.old_file().path(), delta.new_file().path()]
                    .into_iter()
                    .flatten()
                    .map(Path::to_path_buf)
                    .collect();
            }
            Status::INDEX_NEW
                | Status::INDEX_MODIFIED
                | Status::INDEX_DELETED
                | Status::INDEX_RENAMED
                | Status::INDEX_TYPECHANGE
        }
    };
    if !status.intersects(required) {
        return Err(Error::from_str("file has no changes for the requested index action"));
    }

    let mut index = repository.index()?;
    index.read(true)?;
    for target in &paths {
        if (1..=3).any(|stage| index.get_path(target, stage).is_some()) {
            return Err(Error::from_str("conflicted files cannot be staged or unstaged"));
        }
    }
    match action {
        IndexAction::Stage => {
            for target in &paths {
                match fs::symlink_metadata(workdir.join(target)) {
                    Ok(metadata) if metadata.is_dir() => {
                        return Err(Error::from_str("directories cannot be staged"));
                    }
                    Ok(_) => index.add_path(target)?,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        if index.get_path(target, 0).is_some() {
                            index.remove_path(target)?;
                        }
                    }
                    Err(error) => return Err(Error::from_str(&error.to_string())),
                }
            }
        }
        IndexAction::Unstage => {
            let mut head_index = Index::new()?;
            match repository.head() {
                Ok(head) => head_index.read_tree(&head.peel_to_tree()?)?,
                Err(error) if error.code() == ErrorCode::UnbornBranch => {}
                Err(error) => return Err(error),
            }
            for target in &paths {
                if let Some(entry) = head_index.get_path(target, 0) {
                    index.add(&entry)?;
                } else if index.get_path(target, 0).is_some() {
                    index.remove_path(target)?;
                }
            }
        }
    }
    index.write()
}
