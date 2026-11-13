use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::workspace::git::diff_patch::PatchError;
use crate::workspace::git::error::{GitIoError, RepositoryError};
use crate::workspace::git::index::UpdateIndexError;

pub type Result<T> = std::result::Result<T, GitOperationError>;
use git2::{
    Delta, DiffFindOptions, DiffOptions, ErrorCode, FileMode, Patch, Repository, Status,
    StatusOptions,
};
use serde::{Deserialize, Serialize};

use crate::workspace::git::diff_patch::{DiffPatch, PatchDirection, PatchScope};
use crate::workspace::git::index::{IndexAction, update_index};
use crate::workspace::git::repository::ChangeGroup;

static NEXT_BACKUP: AtomicU64 = AtomicU64::new(0);
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct FileSnapshot {
    bytes: Option<Vec<u8>>,
    mode: u32,
}

impl FileSnapshot {
    fn new(bytes: Option<Vec<u8>>, mode: u32) -> Self {
        Self { bytes, mode }
    }

    fn read(path: &Path) -> Result<Self> {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if !metadata.is_file() || metadata.file_type().is_symlink() {
                    return Err(GitOperationError::non_regular_target());
                }
                Ok(Self::new(
                    Some(fs::read(path).map_err(|source| {
                        GitIoError::new("reading file snapshot", path, source)
                    })?),
                    file_mode(&metadata),
                ))
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::new(None, 0)),
            Err(error) => Err(error)
                .map_err(|source| GitIoError::new("reading snapshot metadata", path, source))
                .map_err(GitOperationError::from),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct DiscardBackupEntry {
    #[serde(skip)]
    directory: PathBuf,
    root: PathBuf,
    path: PathBuf,
}

impl DiscardBackupEntry {
    pub fn new(directory: PathBuf, root: PathBuf, path: PathBuf) -> Self {
        Self { directory, root, path }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<DiscardBackup> {
        let backup = DiscardBackup::load(&self.directory)?;
        if backup.directory() != self.directory
            || backup.root() != self.root
            || backup.path() != self.path
        {
            return Err(GitOperationError::backup_identity_changed());
        }
        Ok(backup)
    }

    fn read(directory: &Path) -> Result<Self> {
        let summary: Self = serde_json::from_reader(BufReader::new(backup_manifest(directory)?))
            .map_err(GitOperationError::reading_discard_backup_summary)?;
        if !summary.root.is_absolute() {
            return Err(GitOperationError::relative_backup_root());
        }
        validate_relative_path(&summary.path)?;
        Ok(Self::new(
            fs::canonicalize(directory).map_err(|source| {
                GitIoError::new("resolving backup directory", directory, source)
            })?,
            summary.root,
            summary.path,
        ))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscardBackup {
    directory: PathBuf,
    root: PathBuf,
    path: PathBuf,
    original: FileSnapshot,
    resulting: FileSnapshot,
}

impl DiscardBackup {
    fn new(
        directory: PathBuf,
        root: PathBuf,
        path: PathBuf,
        original: FileSnapshot,
        resulting: FileSnapshot,
    ) -> Self {
        Self { directory, root, path, original, resulting }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(directory: &Path) -> Result<Self> {
        let mut backup: Self = serde_json::from_reader(BufReader::new(backup_manifest(directory)?))
            .map_err(|source| GitOperationError::read_backup(directory, source))?;
        backup.directory = fs::canonicalize(directory)
            .map_err(|source| GitIoError::new("resolving backup directory", directory, source))?;
        if !backup.root.is_absolute() {
            return Err(GitOperationError::relative_backup_root());
        }
        validate_relative_path(&backup.path)?;
        Ok(backup)
    }

    pub fn entries() -> Result<Vec<Result<DiscardBackupEntry>>> {
        let base = backup_base()?;
        if !base.exists() {
            return Ok(Vec::new());
        }
        let mut directories = fs::read_dir(&base)
            .map_err(|source| GitIoError::new("listing discard backups", &base, source))?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()
            .map_err(|source| GitIoError::new("reading discard backup entries", &base, source))?;
        directories.sort();
        let mut backups = Vec::new();
        for directory in directories {
            if directory.join("backup.json").exists() {
                backups.push(
                    DiscardBackupEntry::read(&directory)
                        .map_err(|source| GitOperationError::backup_entry(&directory, source)),
                );
            }
        }
        Ok(backups)
    }

    pub fn restore(&self) -> Result<()> {
        self.restore_inner()
            .map_err(|source| GitOperationError::backup_retained(&self.directory, source))
    }

    fn restore_inner(&self) -> Result<()> {
        let repository =
            Repository::open(&self.root).map_err(GitOperationError::opening_backup_repository)?;
        let root = repository_root(&repository)?;
        if root != self.root {
            return Err(GitOperationError::backup_repository_moved());
        }
        validate_target(&repository, &root, &self.path)?;
        let target = root.join(&self.path);
        if FileSnapshot::read(&target)? != self.resulting {
            return Err(GitOperationError::restore_snapshot_changed());
        }
        match &self.original.bytes {
            Some(bytes) => {
                let parent =
                    target.parent().ok_or_else(GitOperationError::missing_target_parent)?;
                fs::create_dir_all(parent).map_err(|source| {
                    GitIoError::new("creating restore directory", parent, source)
                })?;
                validate_target(&repository, &root, &self.path)?;
                let temporary = parent.join(format!(".aries-restore-{}", unique_name()?));
                let mut file = private_file(&temporary)?;
                let installation: Result<()> = (|| {
                    file.write_all(bytes).map_err(|source| {
                        GitIoError::new("writing restore content", &temporary, source)
                    })?;
                    file.sync_all().map_err(|source| {
                        GitIoError::new("syncing restore content", &temporary, source)
                    })?;
                    validate_target(&repository, &root, &self.path)?;
                    if FileSnapshot::read(&target)? != self.resulting {
                        return Err(GitOperationError::restore_install_conflict());
                    }
                    fs::rename(&temporary, &target)
                        .map_err(GitOperationError::installing_restored_content)?;
                    Ok(())
                })();
                if let Err(error) = installation {
                    if let Err(cleanup) = fs::remove_file(&temporary) {
                        return Err(GitOperationError::restore_cleanup(&temporary, error, cleanup));
                    }
                    return Err(error);
                }
                set_mode(&target, self.original.mode)
                    .map_err(GitOperationError::restore_permissions)?;
                file.sync_all().map_err(GitOperationError::restore_file_sync)?;
                sync_directory(parent).map_err(GitOperationError::restore_directory_sync)?;
            },
            None => {
                if self.resulting.bytes.is_some() {
                    if FileSnapshot::read(&target)? != self.resulting {
                        return Err(GitOperationError::restore_removal_conflict());
                    }
                    fs::remove_file(&target)
                        .map_err(GitOperationError::restoring_original_file_absence)?;
                    sync_directory(
                        target.parent().ok_or_else(GitOperationError::missing_target_parent)?,
                    )
                    .map_err(GitOperationError::restore_absence_sync)?;
                }
            },
        }
        if FileSnapshot::read(&target).map_err(GitOperationError::restore_final_snapshot)?
            != self.original
        {
            return Err(GitOperationError::restore_result_mismatch());
        }
        Ok(())
    }

    fn prepare(root: &Path, path: &Path, original: FileSnapshot, patch: &str) -> Result<Self> {
        let base = backup_base()?;
        create_private_directory(&base)?;
        let directory = base.join(unique_name()?);
        create_private_directory(&directory)?;
        let preview = directory.join("preview");
        create_private_directory(&preview)?;
        let target = preview.join(path);
        fs::create_dir_all(target.parent().ok_or_else(GitOperationError::missing_preview_parent)?)
            .map_err(|source| GitIoError::new("creating preview directory", &target, source))?;
        if let Some(bytes) = &original.bytes {
            write_private(&target, bytes)?;
            set_mode(&target, original.mode)?;
        }
        run_apply(&preview, patch, false, true)?;
        run_apply(&preview, patch, false, false)?;
        let mut resulting = FileSnapshot::read(&target)?;
        if original.bytes.is_some() && resulting.bytes.is_some() {
            set_mode(&target, original.mode)?;
            resulting = FileSnapshot::read(&target)?;
        }
        let backup = Self::new(directory, root.to_owned(), path.to_owned(), original, resulting);
        write_private(&backup.directory.join("backup.json"), &serde_json::to_vec(&backup)?)?;
        sync_directory(&backup.directory)?;
        sync_directory(&base)?;
        Ok(backup)
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
        Some(backup) => GitOperationError::backup_retained(&backup.directory, source),
        None => GitOperationError::applying_partial_index_patch(source),
    })?;
    if let Some(backup) = &backup {
        let verification: Result<()> = (|| {
            let target = root.join(path);
            let actual = FileSnapshot::read(&target)?;
            if actual.bytes != backup.resulting.bytes {
                return Err(GitOperationError::discard_result_bytes_mismatch());
            }
            if actual.bytes.is_some() {
                set_mode(&target, backup.resulting.mode)
                    .map_err(GitOperationError::discard_permissions)?;
                File::open(&target)
                    .and_then(|file| file.sync_all())
                    .map_err(|source| GitIoError::new("syncing discarded file", &target, source))?;
            }
            if FileSnapshot::read(&target)? != backup.resulting {
                return Err(GitOperationError::discard_result_mismatch());
            }
            if let Some(parent) = root.join(path).parent() {
                sync_directory(parent)?;
            }
            Ok(())
        })();
        verification
            .map_err(|source| GitOperationError::discard_verification(&backup.directory, source))?;
    }
    Ok(backup)
}

fn validate_patch_for_write(raw: &str) -> Result<()> {
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

fn validate_relative_path(path: &Path) -> Result<()> {
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

fn repository_root(repository: &Repository) -> Result<PathBuf> {
    fs::canonicalize(repository.workdir().ok_or_else(GitOperationError::no_workdir)?)
        .map_err(GitOperationError::resolving_repository_workdir)
}

fn validate_target(repository: &Repository, root: &Path, path: &Path) -> Result<()> {
    validate_target_for_index(repository, root, path, false)
}

fn validate_target_for_index(
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

fn backup_manifest(directory: &Path) -> Result<File> {
    let metadata = fs::symlink_metadata(directory)
        .map_err(|source| GitIoError::new("reading backup directory", directory, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(GitOperationError::invalid_backup_directory());
    }
    let manifest = directory.join("backup.json");
    let metadata = fs::symlink_metadata(&manifest)
        .map_err(|source| GitIoError::new("reading backup manifest metadata", &manifest, source))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(GitOperationError::non_regular_manifest());
    }
    File::open(&manifest)
        .map_err(|source| GitIoError::new("opening backup manifest", &manifest, source))
        .map_err(GitOperationError::from)
}

fn backup_base() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .ok_or_else(GitOperationError::no_local_data_directory)?
        .join("aries-desktop")
        .join("diff-backups"))
}

fn unique_name() -> Result<String> {
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    Ok(format!(
        "{timestamp:039}-{}-{}",
        std::process::id(),
        NEXT_BACKUP.fetch_add(1, Ordering::Relaxed)
    ))
}

fn create_private_directory(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|source| GitIoError::new("creating private directory", path, source))?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| GitIoError::new("reading private directory metadata", path, source))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(GitOperationError::non_directory_backup());
    }
    set_mode(path, 0o700)
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = private_file(path)?;
    file.write_all(bytes)
        .map_err(|source| GitIoError::new("writing snapshot content", path, source))?;
    file.sync_all().map_err(|source| GitIoError::new("syncing private file", path, source))?;
    Ok(())
}

fn private_file(path: &Path) -> Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|source| GitIoError::new("creating private backup file", path, source))
        .map_err(GitOperationError::from)
}

fn file_mode(metadata: &fs::Metadata) -> u32 {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o7777
    }
    #[cfg(not(unix))]
    {
        u32::from(metadata.permissions().readonly())
    }
}

fn set_mode(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|source| GitIoError::new("setting permissions", path, source))?;
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)
            .map_err(|source| GitIoError::new("reading permissions", path, source))?
            .permissions();
        permissions.set_readonly(mode != 0);
        fs::set_permissions(path, permissions)
            .map_err(|source| GitIoError::new("setting permissions", path, source))?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| GitIoError::new("syncing directory", path, source))?;
    Ok(())
}

#[derive(Debug, Error)]
pub enum GitOperationError {
    #[error("target is not a regular file")]
    NonRegularTarget,
    #[error("backup identity changed since listing")]
    BackupIdentityChanged,
    #[error("backup root is not absolute")]
    RelativeBackupRoot,
    #[error("backup repository moved")]
    BackupRepositoryMoved,
    #[error("丢弃后文件已变化，未开始恢复；请先保存当前修改并核对备份")]
    RestoreSnapshotChanged,
    #[error("恢复期间文件已变化，未安装恢复内容；请暂停编辑后重试")]
    RestoreInstallConflict,
    #[error("恢复期间文件已变化，未移除文件；请暂停编辑后重试")]
    RestoreRemovalConflict,
    #[error("恢复已执行，但文件与备份不一致；可能存在并发编辑，请检查当前内容")]
    RestoreResultMismatch,
    #[error("file has no changes or ambiguous rename status")]
    AmbiguousFileChanges,
    #[error("action is not valid for this change group")]
    InvalidActionGroup,
    #[error("diff 已过期，请刷新后再修改文件")]
    StaleDiff,
    #[error("invalid file index action")]
    InvalidFileIndexAction,
    #[error("暂存区操作前 diff 已变化，未执行操作；请刷新后重试")]
    DiffChangedBeforeIndexOperation,
    #[error("应用 patch 前 diff 已变化，未执行操作；请刷新后重试")]
    DiffChangedBeforeApply,
    #[error("应用 patch 前工作区已变化，未执行操作；请暂停编辑并刷新后重试")]
    WorktreeChangedBeforeApply,
    #[error("discard result bytes mismatch")]
    DiscardResultBytesMismatch,
    #[error("discard result mismatch")]
    DiscardResultMismatch,
    #[error("expected one file patch")]
    ExpectedOneFilePatch,
    #[error("binary write operations are not supported")]
    BinaryWriteUnsupported,
    #[error("rename and type changes are not supported")]
    UnsupportedDelta,
    #[error("symlink and submodule operations are not supported")]
    UnsupportedFileMode,
    #[error("mode-only and rename operations are not supported")]
    UnsupportedMetadata,
    #[error("expected repository-relative file path")]
    InvalidRelativePath,
    #[error("path traversal is not permitted")]
    PathTraversal,
    #[error("Git metadata paths are not permitted")]
    MetadataPath,
    #[error("target is inside Git metadata")]
    MetadataTarget,
    #[error("symlink targets and ancestors are not permitted")]
    SymlinkTarget,
    #[error("target ancestor is not a directory")]
    NonDirectoryAncestor,
    #[error("nested repositories are not permitted")]
    NestedRepository,
    #[error("directories and special files are not permitted")]
    SpecialFileTarget,
    #[error("symlinks and submodules are not permitted")]
    UnsupportedIndexMode,
    #[error("文件存在冲突，请先解决冲突再操作")]
    ConflictedFile,
    #[error("conflict and type changes are not permitted")]
    UnsupportedStatus,
    #[error("重命名不支持部分操作或丢弃，请使用整个文件的暂存／取消暂存")]
    PartialRenameUnsupported,
    #[error("invalid backup directory")]
    InvalidBackupDirectory,
    #[error("backup manifest is not a regular file")]
    NonRegularManifest,
    #[error("backup directory is not a real directory")]
    NonDirectoryBackup,
    #[error("Git 写入状态锁已损坏，已停止写入；请检查仓库状态并重启应用")]
    WriteLockPoisoned,
    #[error("target has no parent")]
    MissingTargetParent,
    #[error("preview target has no parent")]
    MissingPreviewParent,
    #[error("Git patch unavailable")]
    PatchUnavailable,
    #[error("missing file delta")]
    MissingFileDelta,
    #[error("repository has no workdir")]
    NoWorkdir,
    #[error("git apply stdin unavailable")]
    ApplyStdinUnavailable,
    #[error("local data directory is unavailable")]
    NoLocalDataDirectory,
    #[error("reading discard backup summary")]
    ReadingDiscardBackupSummary { source: serde_json::Error },
    #[error("opening backup repository")]
    OpeningBackupRepository { source: git2::Error },
    #[error("installing restored content")]
    InstallingRestoredContent { source: std::io::Error },
    #[error("内容已恢复，但权限恢复失败；请检查文件权限，不要盲目重试")]
    RestorePermissions { source: Box<GitOperationError> },
    #[error("内容已恢复，但文件同步失败；请检查磁盘及当前内容")]
    RestoreFileSync { source: std::io::Error },
    #[error("内容已恢复，但目录同步失败；请检查磁盘及当前内容")]
    RestoreDirectorySync { source: Box<GitOperationError> },
    #[error("restoring original file absence")]
    RestoringOriginalFileAbsence { source: std::io::Error },
    #[error("文件已恢复为原先不存在的状态，但目录同步失败；请检查磁盘")]
    RestoreAbsenceSync { source: Box<GitOperationError> },
    #[error("恢复已执行，但无法读取最终文件状态；请检查当前内容")]
    RestoreFinalSnapshot { source: Box<GitOperationError> },
    #[error("file patch is not strict UTF-8; display text cannot be used for writes")]
    NonUtf8WritePatch { source: std::string::FromUtf8Error },
    #[error("无法更新 Git 暂存区，请检查仓库状态后重试")]
    UpdatingGitIndex { source: UpdateIndexError },
    #[error("无法创建持久丢弃备份，未应用变更；请检查备份目录权限和磁盘空间")]
    PrepareBackup { source: Box<GitOperationError> },
    #[error("丢弃已执行，但权限恢复失败；请检查文件权限及保留的备份")]
    DiscardPermissions { source: Box<GitOperationError> },
    #[error("resolving repository workdir")]
    ResolvingRepositoryWorkdir { source: std::io::Error },
    #[error("index path is not UTF-8")]
    IndexPathIsNotUtf8 { source: std::str::Utf8Error },
    #[error("starting git apply")]
    StartingGitApply { source: std::io::Error },
    #[error("Git 仓库操作失败，请检查仓库状态后重试")]
    Repository { source: RepositoryError },
    #[error("Git 文件操作失败，请检查路径、权限和磁盘空间")]
    Io { source: GitIoError },
    #[error("serializing discard backup")]
    Serialize { source: serde_json::Error },
    #[error("creating discard backup timestamp")]
    Clock { source: std::time::SystemTimeError },
    #[error("无法处理所选 patch，请刷新 diff 并重新选择变更")]
    Patch { source: PatchError },
    #[error("操作失败，备份保留在 {}；请检查当前文件后再从「恢复丢弃」恢复", directory.display())]
    BackupRetained { directory: PathBuf, source: Box<GitOperationError> },
    #[error("恢复内容未安装，临时文件清理失败（{cleanup}）；恢复数据保留在 {}，请检查后手动处理", temporary.display())]
    RestoreCleanup { temporary: PathBuf, cleanup: std::io::Error, source: Box<GitOperationError> },
    #[error("丢弃已执行，但结果校验失败；备份保留在 {}，请检查当前文件后再恢复", directory.display())]
    DiscardVerification { directory: PathBuf, source: Box<GitOperationError> },
    #[error("暂存区 patch 应用失败，请检查当前暂存区状态后再操作")]
    ApplyingPartialIndexPatch { source: Box<GitOperationError> },
    #[error("无法读取备份记录 {}，请检查备份文件", directory.display())]
    BackupEntry { directory: PathBuf, source: Box<GitOperationError> },
    #[error("无法读取丢弃备份 {}，请检查备份文件", directory.display())]
    ReadBackup { directory: PathBuf, source: serde_json::Error },
    #[error("{count} 条备份记录损坏或无法读取，已跳过；有效记录仍可恢复。详情：\n{details}")]
    BackupEntries { count: usize, details: String },
    #[error("等待 git apply 失败（{}）", apply_phase(*check))]
    WaitApply { check: bool, source: std::io::Error },
    #[error("向 git apply 发送 patch 失败（{}）", apply_phase(*check))]
    SendPatch { check: bool, source: Box<GitOperationError> },
    #[error("git apply 失败，退出状态 {status}（{}）：{stderr}", apply_phase(*check))]
    ApplyFailed { check: bool, status: std::process::ExitStatus, stderr: String },
    #[error("{}: {stderr}", offset_diagnostic(*check))]
    ApplyOffset { check: bool, stderr: String },
    #[error("备份属于其他仓库，请切换到对应仓库再恢复")]
    BackupRepositoryMismatch,
}

impl GitOperationError {
    pub fn backup_entries(count: usize, details: String) -> Self {
        Self::BackupEntries { count, details }
    }

    pub fn non_regular_target() -> Self {
        Self::NonRegularTarget
    }
    pub fn backup_identity_changed() -> Self {
        Self::BackupIdentityChanged
    }
    pub fn relative_backup_root() -> Self {
        Self::RelativeBackupRoot
    }
    pub fn backup_repository_moved() -> Self {
        Self::BackupRepositoryMoved
    }
    pub fn restore_snapshot_changed() -> Self {
        Self::RestoreSnapshotChanged
    }
    pub fn restore_install_conflict() -> Self {
        Self::RestoreInstallConflict
    }
    pub fn restore_removal_conflict() -> Self {
        Self::RestoreRemovalConflict
    }
    pub fn restore_result_mismatch() -> Self {
        Self::RestoreResultMismatch
    }
    pub fn ambiguous_file_changes() -> Self {
        Self::AmbiguousFileChanges
    }
    pub fn invalid_action_group() -> Self {
        Self::InvalidActionGroup
    }
    pub fn stale_diff() -> Self {
        Self::StaleDiff
    }
    pub fn invalid_file_index_action() -> Self {
        Self::InvalidFileIndexAction
    }
    pub fn diff_changed_before_index_operation() -> Self {
        Self::DiffChangedBeforeIndexOperation
    }
    pub fn diff_changed_before_apply() -> Self {
        Self::DiffChangedBeforeApply
    }
    pub fn worktree_changed_before_apply() -> Self {
        Self::WorktreeChangedBeforeApply
    }
    pub fn discard_result_bytes_mismatch() -> Self {
        Self::DiscardResultBytesMismatch
    }
    pub fn discard_result_mismatch() -> Self {
        Self::DiscardResultMismatch
    }
    pub fn expected_one_file_patch() -> Self {
        Self::ExpectedOneFilePatch
    }
    pub fn binary_write_unsupported() -> Self {
        Self::BinaryWriteUnsupported
    }
    pub fn unsupported_delta() -> Self {
        Self::UnsupportedDelta
    }
    pub fn unsupported_file_mode() -> Self {
        Self::UnsupportedFileMode
    }
    pub fn unsupported_metadata() -> Self {
        Self::UnsupportedMetadata
    }
    pub fn invalid_relative_path() -> Self {
        Self::InvalidRelativePath
    }
    pub fn path_traversal() -> Self {
        Self::PathTraversal
    }
    pub fn metadata_path() -> Self {
        Self::MetadataPath
    }
    pub fn metadata_target() -> Self {
        Self::MetadataTarget
    }
    pub fn symlink_target() -> Self {
        Self::SymlinkTarget
    }
    pub fn non_directory_ancestor() -> Self {
        Self::NonDirectoryAncestor
    }
    pub fn nested_repository() -> Self {
        Self::NestedRepository
    }
    pub fn special_file_target() -> Self {
        Self::SpecialFileTarget
    }
    pub fn unsupported_index_mode() -> Self {
        Self::UnsupportedIndexMode
    }
    pub fn conflicted_file() -> Self {
        Self::ConflictedFile
    }
    pub fn unsupported_status() -> Self {
        Self::UnsupportedStatus
    }
    pub fn partial_rename_unsupported() -> Self {
        Self::PartialRenameUnsupported
    }
    pub fn invalid_backup_directory() -> Self {
        Self::InvalidBackupDirectory
    }
    pub fn non_regular_manifest() -> Self {
        Self::NonRegularManifest
    }
    pub fn non_directory_backup() -> Self {
        Self::NonDirectoryBackup
    }
    pub fn write_lock_poisoned() -> Self {
        Self::WriteLockPoisoned
    }
    pub fn missing_target_parent() -> Self {
        Self::MissingTargetParent
    }
    pub fn missing_preview_parent() -> Self {
        Self::MissingPreviewParent
    }
    pub fn patch_unavailable() -> Self {
        Self::PatchUnavailable
    }
    pub fn missing_file_delta() -> Self {
        Self::MissingFileDelta
    }
    pub fn no_workdir() -> Self {
        Self::NoWorkdir
    }
    pub fn apply_stdin_unavailable() -> Self {
        Self::ApplyStdinUnavailable
    }
    pub fn no_local_data_directory() -> Self {
        Self::NoLocalDataDirectory
    }
    pub fn reading_discard_backup_summary(source: serde_json::Error) -> Self {
        Self::ReadingDiscardBackupSummary { source }
    }
    pub fn opening_backup_repository(source: git2::Error) -> Self {
        Self::OpeningBackupRepository { source }
    }
    pub fn installing_restored_content(source: std::io::Error) -> Self {
        Self::InstallingRestoredContent { source }
    }
    pub fn restore_permissions(source: GitOperationError) -> Self {
        Self::RestorePermissions { source: Box::new(source) }
    }
    pub fn restore_file_sync(source: std::io::Error) -> Self {
        Self::RestoreFileSync { source }
    }
    pub fn restore_directory_sync(source: GitOperationError) -> Self {
        Self::RestoreDirectorySync { source: Box::new(source) }
    }
    pub fn restoring_original_file_absence(source: std::io::Error) -> Self {
        Self::RestoringOriginalFileAbsence { source }
    }
    pub fn restore_absence_sync(source: GitOperationError) -> Self {
        Self::RestoreAbsenceSync { source: Box::new(source) }
    }
    pub fn restore_final_snapshot(source: GitOperationError) -> Self {
        Self::RestoreFinalSnapshot { source: Box::new(source) }
    }
    pub fn non_utf8_write_patch(source: std::string::FromUtf8Error) -> Self {
        Self::NonUtf8WritePatch { source }
    }
    pub fn updating_git_index(source: UpdateIndexError) -> Self {
        Self::UpdatingGitIndex { source }
    }
    pub fn prepare_backup(source: GitOperationError) -> Self {
        Self::PrepareBackup { source: Box::new(source) }
    }
    pub fn discard_permissions(source: GitOperationError) -> Self {
        Self::DiscardPermissions { source: Box::new(source) }
    }
    pub fn resolving_repository_workdir(source: std::io::Error) -> Self {
        Self::ResolvingRepositoryWorkdir { source }
    }
    pub fn index_path_is_not_utf8(source: std::str::Utf8Error) -> Self {
        Self::IndexPathIsNotUtf8 { source }
    }
    pub fn starting_git_apply(source: std::io::Error) -> Self {
        Self::StartingGitApply { source }
    }
    pub fn backup_retained(directory: &Path, source: GitOperationError) -> Self {
        Self::BackupRetained { directory: directory.to_owned(), source: Box::new(source) }
    }
    pub fn discard_verification(directory: &Path, source: GitOperationError) -> Self {
        Self::DiscardVerification { directory: directory.to_owned(), source: Box::new(source) }
    }
    pub fn backup_entry(directory: &Path, source: GitOperationError) -> Self {
        Self::BackupEntry { directory: directory.to_owned(), source: Box::new(source) }
    }
    pub fn restore_cleanup(
        temporary: &Path,
        source: GitOperationError,
        cleanup: std::io::Error,
    ) -> Self {
        Self::RestoreCleanup { temporary: temporary.to_owned(), source: Box::new(source), cleanup }
    }
    pub fn applying_partial_index_patch(source: GitOperationError) -> Self {
        Self::ApplyingPartialIndexPatch { source: Box::new(source) }
    }
    pub fn read_backup(directory: &Path, source: serde_json::Error) -> Self {
        Self::ReadBackup { directory: directory.to_owned(), source }
    }
    pub fn wait_apply(check: bool, source: std::io::Error) -> Self {
        Self::WaitApply { check, source }
    }
    pub fn send_patch(check: bool, source: GitOperationError) -> Self {
        Self::SendPatch { check, source: Box::new(source) }
    }
    pub fn apply_failed(check: bool, status: std::process::ExitStatus, stderr: String) -> Self {
        Self::ApplyFailed { check, status, stderr }
    }
    pub fn apply_offset(check: bool, stderr: String) -> Self {
        Self::ApplyOffset { check, stderr }
    }
    pub fn backup_repository_mismatch() -> Self {
        Self::BackupRepositoryMismatch
    }
}

fn apply_phase(check: bool) -> &'static str {
    if check {
        "预检查；未应用变更，请刷新后重试"
    } else {
        "已尝试写入；请检查当前状态"
    }
}
impl From<RepositoryError> for GitOperationError {
    fn from(source: RepositoryError) -> Self {
        Self::Repository { source }
    }
}
impl From<GitIoError> for GitOperationError {
    fn from(source: GitIoError) -> Self {
        Self::Io { source }
    }
}
impl From<serde_json::Error> for GitOperationError {
    fn from(source: serde_json::Error) -> Self {
        Self::Serialize { source }
    }
}
impl From<std::time::SystemTimeError> for GitOperationError {
    fn from(source: std::time::SystemTimeError) -> Self {
        Self::Clock { source }
    }
}
impl From<PatchError> for GitOperationError {
    fn from(source: PatchError) -> Self {
        Self::Patch { source }
    }
}

fn offset_diagnostic(check: bool) -> &'static str {
    if check {
        "git apply 预检查需要块偏移，未应用变更；请刷新 diff 后重试"
    } else {
        "git apply 已使用块偏移应用 patch；请检查当前状态，不要盲目重试"
    }
}
