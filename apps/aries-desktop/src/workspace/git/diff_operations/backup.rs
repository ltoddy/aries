use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use git2::Repository;
use serde::{Deserialize, Serialize};

use crate::workspace::git::error::GitIoError;

use crate::workspace::git::diff_operations::error::{GitOperationError, Result};
use crate::workspace::git::diff_operations::run_apply;
use crate::workspace::git::diff_operations::validate::{
    repository_root, validate_relative_path, validate_target,
};

static NEXT_BACKUP: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSnapshot {
    bytes: Option<Vec<u8>>,
    mode: u32,
}

impl FileSnapshot {
    fn new(bytes: Option<Vec<u8>>, mode: u32) -> Self {
        Self { bytes, mode }
    }

    pub fn read(path: &Path) -> Result<Self> {
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

    pub fn prepare(root: &Path, path: &Path, original: FileSnapshot, patch: &str) -> Result<Self> {
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

pub fn verify_discard(root: &Path, path: &Path, backup: &DiscardBackup) -> Result<()> {
    let target = root.join(path);
    let actual = FileSnapshot::read(&target)?;
    if actual.bytes != backup.resulting.bytes {
        return Err(GitOperationError::discard_result_bytes_mismatch());
    }
    if actual.bytes.is_some() {
        set_mode(&target, backup.resulting.mode).map_err(GitOperationError::discard_permissions)?;
        File::open(&target)
            .and_then(|file| file.sync_all())
            .map_err(|source| GitIoError::new("syncing discarded file", &target, source))?;
    }
    if FileSnapshot::read(&target)? != backup.resulting {
        return Err(GitOperationError::discard_result_mismatch());
    }
    if let Some(parent) = target.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}
