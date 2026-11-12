use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail, ensure};
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
    let _guard = GIT_WRITE.lock().map_err(|_| anyhow::anyhow!("Git write lock poisoned"))?;
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
                ensure!(
                    metadata.is_file() && !metadata.file_type().is_symlink(),
                    "target is not a regular file"
                );
                Ok(Self::new(
                    Some(fs::read(path).context("reading file snapshot")?),
                    file_mode(&metadata),
                ))
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::new(None, 0)),
            Err(error) => Err(error).context("reading snapshot metadata"),
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
        ensure!(
            backup.directory() == self.directory
                && backup.root() == self.root
                && backup.path() == self.path,
            "backup identity changed since listing"
        );
        Ok(backup)
    }

    fn read(directory: &Path) -> Result<Self> {
        let summary: Self = serde_json::from_reader(BufReader::new(backup_manifest(directory)?))
            .context("reading discard backup summary")?;
        ensure!(summary.root.is_absolute(), "backup root is not absolute");
        validate_relative_path(&summary.path)?;
        Ok(Self::new(fs::canonicalize(directory)?, summary.root, summary.path))
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
            .with_context(|| format!("reading discard backup at {}", directory.display()))?;
        backup.directory = fs::canonicalize(directory)?;
        ensure!(backup.root.is_absolute(), "backup root is not absolute");
        validate_relative_path(&backup.path)?;
        Ok(backup)
    }

    pub fn entries() -> Result<Vec<Result<DiscardBackupEntry>>> {
        let base = backup_base()?;
        if !base.exists() {
            return Ok(Vec::new());
        }
        let mut directories = fs::read_dir(base)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        directories.sort();
        let mut backups = Vec::new();
        for directory in directories {
            if directory.join("backup.json").exists() {
                backups.push(
                    DiscardBackupEntry::read(&directory).with_context(|| {
                        format!("reading backup entry at {}", directory.display())
                    }),
                );
            }
        }
        Ok(backups)
    }

    pub fn restore(&self) -> Result<()> {
        self.restore_inner().map_err(|error| {
            let message = format!("{error:#}; backup retained at {}", self.directory.display());
            error.context(message)
        })
    }

    fn restore_inner(&self) -> Result<()> {
        let repository = Repository::open(&self.root).context("opening backup repository")?;
        let root = repository_root(&repository)?;
        ensure!(root == self.root, "backup repository moved");
        validate_target(&repository, &root, &self.path)?;
        let target = root.join(&self.path);
        ensure!(
            FileSnapshot::read(&target)? == self.resulting,
            "file snapshot changed after discard; restore not started"
        );
        match &self.original.bytes {
            Some(bytes) => {
                let parent = target.parent().context("target has no parent")?;
                fs::create_dir_all(parent)?;
                validate_target(&repository, &root, &self.path)?;
                let temporary = parent.join(format!(".aries-restore-{}", unique_name()?));
                let mut file = private_file(&temporary)?;
                let installation: Result<()> = (|| {
                    file.write_all(bytes)?;
                    file.sync_all()?;
                    validate_target(&repository, &root, &self.path)?;
                    ensure!(
                        FileSnapshot::read(&target)? == self.resulting,
                        "file snapshot changed during restore; content not installed"
                    );
                    fs::rename(&temporary, &target).context("installing restored content")?;
                    Ok(())
                })();
                if let Err(error) = installation {
                    if let Err(cleanup) = fs::remove_file(&temporary) {
                        return Err(error).with_context(|| {
                            format!(
                                "restore not installed; cleanup failed ({cleanup}); private recovery data retained at {}",
                                temporary.display()
                            )
                        });
                    }
                    return Err(error);
                }
                set_mode(&target, self.original.mode)
                    .context("content already restored; restoring permissions failed")?;
                file.sync_all()
                    .context("content already restored; syncing restored file failed")?;
                sync_directory(parent)
                    .context("content already restored; syncing directory failed")?;
            },
            None => {
                if self.resulting.bytes.is_some() {
                    ensure!(
                        FileSnapshot::read(&target)? == self.resulting,
                        "file snapshot changed during restore; removal not started"
                    );
                    fs::remove_file(&target).context("restoring original file absence")?;
                    sync_directory(target.parent().context("target has no parent")?).context(
                        "original file absence already restored; syncing directory failed",
                    )?;
                }
            },
        }
        ensure!(
            FileSnapshot::read(&target)
                .context("restore completed; reading final snapshot failed")?
                == self.original,
            "restore completed but final snapshot does not match backup; possible concurrent edit"
        );
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
        fs::create_dir_all(target.parent().context("preview target has no parent")?)?;
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
    let repository = Repository::discover(cwd).context("discovering Git repository")?;
    let mut options = DiffOptions::new();
    options
        .update_index(false)
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .show_untracked_content(true);
    let mut diff = match group {
        ChangeGroup::Staged => {
            let tree = match repository.head() {
                Ok(head) => Some(head.peel_to_tree()?),
                Err(error) if error.code() == ErrorCode::UnbornBranch => None,
                Err(error) => return Err(error.into()),
            };
            repository.diff_tree_to_index(tree.as_ref(), None, Some(&mut options))?
        },
        ChangeGroup::Unstaged => repository.diff_index_to_workdir(None, Some(&mut options))?,
    };
    let mut find = DiffFindOptions::new();
    find.renames(true).for_untracked(true);
    diff.find_similar(Some(&mut find))?;
    let mut output = Vec::new();
    let mut matches = 0;
    for (index, delta) in diff.deltas().enumerate() {
        if delta.old_file().path() != Some(path) && delta.new_file().path() != Some(path) {
            continue;
        }
        matches += 1;
        let mut patch = Patch::from_diff(&diff, index)?.context("Git patch unavailable")?;
        output.extend_from_slice(&patch.to_buf()?);
    }
    ensure!(matches == 1, "file has no changes or ambiguous rename status");
    String::from_utf8(output)
        .context("file patch is not strict UTF-8; display text cannot be used for writes")
}

pub fn execute(
    cwd: &Path,
    path: &Path,
    group: ChangeGroup,
    expected_patch: &str,
    scope: &PatchScope,
    action: DiffAction,
) -> Result<Option<DiscardBackup>> {
    ensure!(
        matches!(
            (group, action),
            (ChangeGroup::Unstaged, DiffAction::Stage | DiffAction::Discard)
                | (ChangeGroup::Staged, DiffAction::Unstage)
        ),
        "action is not valid for this change group"
    );
    let repository = Repository::discover(cwd).context("discovering Git repository")?;
    let root = repository_root(&repository)?;
    let file_index = matches!(scope, PatchScope::File) && action != DiffAction::Discard;
    validate_target_for_index(&repository, &root, path, file_index)?;
    let current = read_file_patch(&root, path, group)?;
    ensure!(current == expected_patch, "diff is stale; refresh before modifying files");
    if file_index {
        let index_action = match action {
            DiffAction::Stage => IndexAction::Stage,
            DiffAction::Unstage => IndexAction::Unstage,
            DiffAction::Discard => bail!("invalid file index action"),
        };
        ensure!(
            read_file_patch(&root, path, group)? == current,
            "diff changed before index operation"
        );
        validate_target_for_index(&repository, &root, path, true)?;
        update_index(&root, path, index_action).context("updating Git index")?;
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
                .context("preparing durable discard backup; no changes applied")?,
        )
    } else {
        None
    };
    validate_target(&repository, &root, path)?;
    ensure!(read_file_patch(&root, path, group)? == current, "diff changed before apply");
    ensure!(FileSnapshot::read(&root.join(path))? == original, "worktree changed before apply");
    run_apply(&root, &patch, cached, false).map_err(|error| {
        let message = match &backup {
            Some(backup) => {
                format!("{error:#}; backup retained at {}", backup.directory.display())
            },
            None => format!("applying partial index patch: {error:#}"),
        };
        error.context(message)
    })?;
    if let Some(backup) = &backup {
        let verification: Result<()> = (|| {
            let target = root.join(path);
            let actual = FileSnapshot::read(&target)?;
            ensure!(actual.bytes == backup.resulting.bytes, "discard result bytes mismatch");
            if actual.bytes.is_some() {
                set_mode(&target, backup.resulting.mode)
                    .context("discard content already applied; restoring permissions failed")?;
                File::open(&target)?.sync_all()?;
            }
            ensure!(FileSnapshot::read(&target)? == backup.resulting, "discard result mismatch");
            if let Some(parent) = root.join(path).parent() {
                sync_directory(parent)?;
            }
            Ok(())
        })();
        verification.map_err(|error| {
            let message = format!(
                "discard already applied; post-apply verification failed: {error:#}; backup retained at {}",
                backup.directory.display()
            );
            error.context(message)
        })?;
    }
    Ok(backup)
}

fn validate_patch_for_write(raw: &str) -> Result<()> {
    let diff = git2::Diff::from_buffer(raw.as_bytes())?;
    ensure!(diff.deltas().len() == 1, "expected one file patch");
    let delta = diff.get_delta(0).context("missing file delta")?;
    ensure!(!delta.flags().is_binary(), "binary write operations are not supported");
    ensure!(
        matches!(
            delta.status(),
            Delta::Added | Delta::Deleted | Delta::Modified | Delta::Untracked
        ),
        "rename and type changes are not supported"
    );
    for mode in [delta.old_file().mode(), delta.new_file().mode()] {
        ensure!(
            matches!(mode, FileMode::Unreadable | FileMode::Blob | FileMode::BlobExecutable),
            "symlink and submodule operations are not supported"
        );
    }
    ensure!(
        !raw.lines().any(|line| line.starts_with("old mode ")
            || line.starts_with("new mode ")
            || line.starts_with("rename ")
            || line.starts_with("similarity ")),
        "mode-only and rename operations are not supported"
    );
    Ok(())
}

fn validate_relative_path(path: &Path) -> Result<()> {
    ensure!(
        !path.as_os_str().is_empty() && !path.as_os_str().as_encoded_bytes().ends_with(b"/"),
        "expected repository-relative file path"
    );
    for component in path.components() {
        let Component::Normal(name) = component else { bail!("path traversal is not permitted") };
        ensure!(!name.eq_ignore_ascii_case(".git"), "Git metadata paths are not permitted");
    }
    Ok(())
}

fn repository_root(repository: &Repository) -> Result<PathBuf> {
    fs::canonicalize(repository.workdir().context("repository has no workdir")?)
        .context("resolving repository workdir")
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
    let gitdir = fs::canonicalize(repository.path())?;
    let mut target = root.to_owned();
    for component in path.components() {
        target.push(component);
        ensure!(!target.starts_with(&gitdir), "target is inside Git metadata");
        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                ensure!(
                    !metadata.file_type().is_symlink(),
                    "symlink targets and ancestors are not permitted"
                );
                if target != root.join(path) {
                    ensure!(metadata.is_dir(), "target ancestor is not a directory");
                    ensure!(!target.join(".git").exists(), "nested repositories are not permitted");
                } else {
                    ensure!(metadata.is_file(), "directories and special files are not permitted");
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(error).context("checking target path"),
        }
    }
    let mut index = repository.index()?;
    index.read(true)?;
    for entry in index.iter() {
        let entry_path =
            Path::new(std::str::from_utf8(&entry.path).context("index path is not UTF-8")?);
        if path.starts_with(entry_path) {
            ensure!(
                !matches!(entry.mode & 0o170000, 0o120000 | 0o160000),
                "symlinks and submodules are not permitted"
            );
        }
    }
    ensure!(
        !(1..=3).any(|stage| index.get_path(path, stage).is_some()),
        "conflicted files cannot be modified"
    );
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
    let statuses = repository.statuses(Some(&mut options))?;
    for entry in statuses.iter() {
        if [entry.head_to_index(), entry.index_to_workdir()].into_iter().flatten().any(|delta| {
            delta.old_file().path() == Some(path) || delta.new_file().path() == Some(path)
        }) {
            ensure!(
                !entry.status().intersects(
                    Status::CONFLICTED | Status::INDEX_TYPECHANGE | Status::WT_TYPECHANGE
                ),
                "conflict and type changes are not permitted"
            );
            ensure!(
                allow_rename
                    || !entry.status().intersects(Status::INDEX_RENAMED | Status::WT_RENAMED),
                "rename partial operations and discard are not permitted; use whole-file stage/unstage"
            );
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
    let mut child = command.spawn().context("starting git apply")?;
    let phase =
        if check { "check; no changes applied" } else { "write attempted; inspect current state" };
    let input: Result<()> = (|| {
        child.stdin.take().context("git apply stdin unavailable")?.write_all(patch.as_bytes())?;
        Ok(())
    })();
    let output =
        child.wait_with_output().with_context(|| format!("waiting for git apply {phase}"))?;
    input.with_context(|| format!("sending patch to git apply {phase}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    ensure!(
        output.status.success(),
        "git apply {} failed{}: {stderr}",
        if check { "check" } else { "write" },
        if check { "; no changes applied" } else { "; write attempted, inspect current state" }
    );
    if stderr.lines().any(hunk_has_offset) {
        if check {
            bail!("git apply check requires a hunk offset; no changes applied: {stderr}");
        }
        bail!(
            "git apply already applied the patch with a hunk offset; inspect current state: {stderr}"
        );
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
    let metadata = fs::symlink_metadata(directory).context("reading backup directory")?;
    ensure!(metadata.is_dir() && !metadata.file_type().is_symlink(), "invalid backup directory");
    let manifest = directory.join("backup.json");
    let metadata = fs::symlink_metadata(&manifest)?;
    ensure!(
        metadata.is_file() && !metadata.file_type().is_symlink(),
        "backup manifest is not a regular file"
    );
    File::open(manifest).context("opening backup manifest")
}

fn backup_base() -> Result<PathBuf> {
    Ok(dirs::data_local_dir()
        .context("local data directory is unavailable")?
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
    fs::create_dir_all(path)?;
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_dir() && !metadata.file_type().is_symlink(),
        "backup directory is not a real directory"
    );
    set_mode(path, 0o700)
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = private_file(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
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
    options.open(path).context("creating private backup file")
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
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    {
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_readonly(mode != 0);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    Ok(())
}
