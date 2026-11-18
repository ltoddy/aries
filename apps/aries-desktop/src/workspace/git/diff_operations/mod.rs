use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};

use git2::{DiffFindOptions, DiffOptions, ErrorCode, Patch, Repository};

use crate::workspace::git::diff_patch::{DiffPatch, PatchDirection, PatchScope};
use crate::workspace::git::error::{GitIoError, RepositoryError};
use crate::workspace::git::index::{IndexAction, update_index};
use crate::workspace::git::repository::ChangeGroup;

mod backup;
mod error;
mod validate;

pub use self::backup::{DiscardBackup, DiscardBackupEntry};
use self::backup::{FileSnapshot, verify_discard};
pub use self::error::{GitOperationError, Result};
use self::validate::{
    repository_root, validate_patch_for_write, validate_target, validate_target_for_index,
};

static GIT_WRITE: Mutex<()> = Mutex::new(());

pub fn with_git_write<T>(operation: impl FnOnce() -> Result<T>) -> Result<T> {
    let _guard = GIT_WRITE.lock().map_err(|_| GitOperationError::write_lock_poisoned())?;
    operation()
}

#[derive(Clone)]
pub struct DiffRequest {
    directory: PathBuf,
    path: PathBuf,
    group: ChangeGroup,
    patch: Arc<String>,
    scope: PatchScope,
    action: DiffAction,
}

impl DiffRequest {
    pub fn new(
        directory: PathBuf,
        path: PathBuf,
        group: ChangeGroup,
        patch: Arc<String>,
        scope: PatchScope,
        action: DiffAction,
    ) -> Self {
        Self { directory, path, group, patch, scope, action }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn action(&self) -> DiffAction {
        self.action
    }

    pub fn execute(&self) -> Result<Option<DiscardBackup>> {
        with_git_write(|| {
            execute(&self.directory, &self.path, self.group, &self.patch, &self.scope, self.action)
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffAction {
    Stage,
    Unstage,
    Discard,
}

impl DiffAction {
    pub fn stage() -> Self {
        Self::Stage
    }

    pub fn unstage() -> Self {
        Self::Unstage
    }

    pub fn discard() -> Self {
        Self::Discard
    }
}

pub fn read_file_patch(cwd: &Path, path: &Path, group: ChangeGroup) -> Result<String> {
    let repository = Repository::discover(cwd)
        .map_err(|source| RepositoryError::git("discovering Git repository", cwd, source))?;
    let mut options = DiffOptions::new();
    options
        .update_index(false)
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true);
    let mut diff = match group {
        ChangeGroup::Staged => {
            let tree = match repository.head() {
                Ok(head) => Some(
                    head.peel_to_tree()
                        .map_err(|source| RepositoryError::git("reading HEAD tree", cwd, source))?,
                ),
                Err(error) if error.code() == ErrorCode::UnbornBranch => None,
                Err(error) => return Err(RepositoryError::git("reading HEAD", cwd, error).into()),
            };
            repository
                .diff_tree_to_index(tree.as_ref(), None, Some(&mut options))
                .map_err(|source| RepositoryError::git("reading staged diff", path, source))?
        },
        ChangeGroup::Unstaged => repository
            .diff_index_to_workdir(None, Some(&mut options))
            .map_err(|source| RepositoryError::git("reading unstaged diff", path, source))?,
    };
    let mut find = DiffFindOptions::new();
    find.renames(true).for_untracked(true);
    diff.find_similar(Some(&mut find))
        .map_err(|source| RepositoryError::git("finding renames", path, source))?;
    let mut output = Vec::new();
    let mut matches = 0;
    for (index, delta) in diff.deltas().enumerate() {
        if delta.old_file().path() != Some(path) && delta.new_file().path() != Some(path) {
            continue;
        }
        matches += 1;
        let mut patch = Patch::from_diff(&diff, index)
            .map_err(|source| RepositoryError::git("reading Git patch", path, source))?
            .ok_or_else(GitOperationError::patch_unavailable)?;
        output.extend_from_slice(
            &patch
                .to_buf()
                .map_err(|source| RepositoryError::git("serializing Git patch", path, source))?,
        );
    }
    if matches != 1 {
        return Err(GitOperationError::ambiguous_file_changes());
    }
    String::from_utf8(output).map_err(GitOperationError::non_utf8_write_patch)
}

pub fn execute(
    cwd: &Path,
    path: &Path,
    group: ChangeGroup,
    expected_patch: &str,
    scope: &PatchScope,
    action: DiffAction,
) -> Result<Option<DiscardBackup>> {
    if !matches!(
        (group, action),
        (ChangeGroup::Unstaged, DiffAction::Stage | DiffAction::Discard)
            | (ChangeGroup::Staged, DiffAction::Unstage)
    ) {
        return Err(GitOperationError::invalid_action_group());
    }
    let repository = Repository::discover(cwd)
        .map_err(|source| RepositoryError::git("discovering Git repository", cwd, source))?;
    let root = repository_root(&repository)?;
    let file_index = matches!(scope, PatchScope::File) && action != DiffAction::Discard;
    validate_target_for_index(&repository, &root, path, file_index)?;
    let current = read_file_patch(&root, path, group)?;
    if current != expected_patch {
        return Err(GitOperationError::stale_diff());
    }
    if file_index {
        let index_action = match action {
            DiffAction::Stage => IndexAction::Stage,
            DiffAction::Unstage => IndexAction::Unstage,
            DiffAction::Discard => return Err(GitOperationError::invalid_file_index_action()),
        };
        if read_file_patch(&root, path, group)? != current {
            return Err(GitOperationError::diff_changed_before_index_operation());
        }
        validate_target_for_index(&repository, &root, path, true)?;
        update_index(&root, path, index_action).map_err(GitOperationError::updating_git_index)?;
        return Ok(None);
    }
    validate_patch_for_write(&current)?;
    let model = DiffPatch::parse(&current)?;
    let direction = if action == DiffAction::Stage {
        PatchDirection::forward()
    } else {
        PatchDirection::reverse()
    };
    let patch = model.selected_patch(scope, direction)?;
    let cached = action != DiffAction::Discard;
    run_apply(&root, &patch, cached, true)?;
    let original = FileSnapshot::read(&root.join(path))?;
    let backup = if action == DiffAction::Discard {
        Some(
            DiscardBackup::prepare(&root, path, original.clone(), &patch)
                .map_err(GitOperationError::prepare_backup)?,
        )
    } else {
        None
    };
    validate_target(&repository, &root, path)?;
    if read_file_patch(&root, path, group)? != current {
        return Err(GitOperationError::diff_changed_before_apply());
    }
    if FileSnapshot::read(&root.join(path))? != original {
        return Err(GitOperationError::worktree_changed_before_apply());
    }
    run_apply(&root, &patch, cached, false).map_err(|source| match &backup {
        Some(backup) => GitOperationError::backup_retained(backup.directory(), source),
        None => GitOperationError::applying_partial_index_patch(source),
    })?;
    if let Some(backup) = &backup {
        verify_discard(&root, path, backup).map_err(|source| {
            GitOperationError::discard_verification(backup.directory(), source)
        })?;
    }
    Ok(backup)
}

fn run_apply(root: &Path, patch: &str, cached: bool, check: bool) -> Result<()> {
    let mut command = Command::new("git");
    for (key, _) in std::env::vars_os() {
        if key.as_encoded_bytes().starts_with(b"GIT_") {
            command.env_remove(key);
        }
    }
    command.env("LC_ALL", "C");
    command.arg("-C").arg(root).args([
        "-c",
        "core.autocrlf=false",
        "apply",
        "--verbose",
        "--whitespace=nowarn",
    ]);
    if cached {
        command.arg("--cached");
    }
    if check {
        command.arg("--check");
    }
    command.arg("-").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn().map_err(GitOperationError::starting_git_apply)?;

    let input: Result<()> = (|| {
        child
            .stdin
            .take()
            .ok_or_else(GitOperationError::apply_stdin_unavailable)?
            .write_all(patch.as_bytes())
            .map_err(|source| GitIoError::new("sending patch", root, source))?;
        Ok(())
    })();
    let output =
        child.wait_with_output().map_err(|source| GitOperationError::wait_apply(check, source))?;
    input.map_err(|source| GitOperationError::send_patch(check, source))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        return Err(GitOperationError::apply_failed(check, output.status, stderr.into_owned()));
    }
    if stderr.lines().any(hunk_has_offset) {
        return Err(GitOperationError::apply_offset(check, stderr.into_owned()));
    }
    Ok(())
}

fn hunk_has_offset(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("Hunk #") else { return false };
    let Some((number, rest)) = rest.split_once(" succeeded at ") else { return false };
    let Some((position, diagnostic)) = rest.split_once(" (offset ") else { return false };
    let Some(offset) =
        diagnostic.strip_suffix(" lines).").or_else(|| diagnostic.strip_suffix(" line)."))
    else {
        return false;
    };
    number.parse::<u32>().is_ok()
        && position.parse::<u32>().is_ok()
        && offset.parse::<i64>().is_ok()
}
