use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::workspace::git::diff_patch::PatchError;
use crate::workspace::git::error::{GitIoError, RepositoryError};
use crate::workspace::git::index::UpdateIndexError;

pub type Result<T> = std::result::Result<T, GitOperationError>;

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
