use std::fs;
use std::path::{Component, Path, PathBuf};

use git2::{Delta, FileMode, Repository, Status, StatusOptions};

use crate::workspace::git::diff_patch::PatchError;
use crate::workspace::git::error::{GitIoError, RepositoryError};

use crate::workspace::git::diff_operations::error::{GitOperationError, Result};

pub fn validate_patch_for_write(raw: &str) -> Result<()> {
    let diff = git2::Diff::from_buffer(raw.as_bytes())
        .map_err(|source| PatchError::git("validating patch for write", source))?;
    if diff.deltas().len() != 1 {
        return Err(GitOperationError::expected_one_file_patch());
    }
    let delta = diff.get_delta(0).ok_or_else(GitOperationError::missing_file_delta)?;
    if delta.flags().is_binary() {
        return Err(GitOperationError::binary_write_unsupported());
    }
    if !matches!(delta.status(), Delta::Added | Delta::Deleted | Delta::Modified | Delta::Untracked)
    {
        return Err(GitOperationError::unsupported_delta());
    }
    for mode in [delta.old_file().mode(), delta.new_file().mode()] {
        if !matches!(mode, FileMode::Unreadable | FileMode::Blob | FileMode::BlobExecutable) {
            return Err(GitOperationError::unsupported_file_mode());
        }
    }
    if raw.lines().any(|line| {
        line.starts_with("old mode ")
            || line.starts_with("new mode ")
            || line.starts_with("rename ")
            || line.starts_with("similarity ")
    }) {
        return Err(GitOperationError::unsupported_metadata());
    }
    Ok(())
}

pub fn validate_relative_path(path: &Path) -> Result<()> {
    if path.as_os_str().is_empty() || path.as_os_str().as_encoded_bytes().ends_with(b"/") {
        return Err(GitOperationError::invalid_relative_path());
    }
    for component in path.components() {
        let Component::Normal(name) = component else {
            return Err(GitOperationError::path_traversal());
        };
        if name.eq_ignore_ascii_case(".git") {
            return Err(GitOperationError::metadata_path());
        }
    }
    Ok(())
}

pub fn repository_root(repository: &Repository) -> Result<PathBuf> {
    fs::canonicalize(repository.workdir().ok_or_else(GitOperationError::no_workdir)?)
        .map_err(GitOperationError::resolving_repository_workdir)
}

pub fn validate_target(repository: &Repository, root: &Path, path: &Path) -> Result<()> {
    validate_target_for_index(repository, root, path, false)
}

pub fn validate_target_for_index(
    repository: &Repository,
    root: &Path,
    path: &Path,
    allow_rename: bool,
) -> Result<()> {
    validate_target_path(repository, root, path)?;
    validate_target_status(repository, root, path, allow_rename)
}

fn validate_target_path(repository: &Repository, root: &Path, path: &Path) -> Result<()> {
    validate_relative_path(path)?;
    let gitdir = fs::canonicalize(repository.path()).map_err(|source| {
        GitIoError::new("resolving Git metadata directory", repository.path(), source)
    })?;
    let mut target = root.to_owned();
    for component in path.components() {
        target.push(component);
        if target.starts_with(&gitdir) {
            return Err(GitOperationError::metadata_target());
        }
        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(GitOperationError::symlink_target());
                }
                if target != root.join(path) {
                    if !metadata.is_dir() {
                        return Err(GitOperationError::non_directory_ancestor());
                    }
                    if target.join(".git").exists() {
                        return Err(GitOperationError::nested_repository());
                    }
                } else {
                    if !metadata.is_file() {
                        return Err(GitOperationError::special_file_target());
                    }
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => {
                return Err(error)
                    .map_err(|source| GitIoError::new("checking target path", &target, source))
                    .map_err(GitOperationError::from);
            },
        }
    }
    let mut index = repository
        .index()
        .map_err(|source| RepositoryError::git("opening Git index", root, source))?;
    index.read(true).map_err(|source| RepositoryError::git("reading Git index", root, source))?;
    for entry in index.iter() {
        let entry_path = Path::new(
            std::str::from_utf8(&entry.path).map_err(GitOperationError::index_path_is_not_utf8)?,
        );
        if path.starts_with(entry_path) && matches!(entry.mode & 0o170000, 0o120000 | 0o160000) {
            return Err(GitOperationError::unsupported_index_mode());
        }
    }
    if (1..=3).any(|stage| index.get_path(path, stage).is_some()) {
        return Err(GitOperationError::conflicted_file());
    }
    Ok(())
}

fn validate_target_status(
    repository: &Repository,
    root: &Path,
    path: &Path,
    allow_rename: bool,
) -> Result<()> {
    let mut options = StatusOptions::new();
    options
        .update_index(false)
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repository
        .statuses(Some(&mut options))
        .map_err(|source| RepositoryError::git("reading target status", path, source))?;
    for entry in statuses.iter() {
        if [entry.head_to_index(), entry.index_to_workdir()].into_iter().flatten().any(|delta| {
            delta.old_file().path() == Some(path) || delta.new_file().path() == Some(path)
        }) {
            if entry
                .status()
                .intersects(Status::CONFLICTED | Status::INDEX_TYPECHANGE | Status::WT_TYPECHANGE)
            {
                return Err(GitOperationError::unsupported_status());
            }
            if !allow_rename
                && entry.status().intersects(Status::INDEX_RENAMED | Status::WT_RENAMED)
            {
                return Err(GitOperationError::partial_rename_unsupported());
            }
            if allow_rename {
                for delta in [entry.head_to_index(), entry.index_to_workdir()].into_iter().flatten()
                {
                    if delta.status() == Delta::Renamed {
                        for renamed in
                            [delta.old_file().path(), delta.new_file().path()].into_iter().flatten()
                        {
                            if renamed != path {
                                validate_target_path(repository, root, renamed)?;
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
