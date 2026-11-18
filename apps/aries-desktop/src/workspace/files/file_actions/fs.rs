#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::ffi::{OsStr, OsString};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::fs::File;
use std::fs::{self, Metadata};
use std::io;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

#[cfg(any(target_os = "macos", target_os = "linux"))]
use rustix::fs::{self as fd_fs, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat};

use crate::workspace::files::file_actions::FileActionError;

pub fn require_absolute(path: &Path) -> Result<(), FileActionError> {
    let bytes = path.as_os_str().as_encoded_bytes();
    if !path.is_absolute()
        || bytes.ends_with(b"/.")
        || bytes.ends_with(b"/..")
        || (cfg!(windows)
            && bytes
                .rsplit(|byte| *byte == b'\\' || *byte == b'/')
                .next()
                .is_some_and(|name| name == b"." || name == b".."))
        || path.components().any(|component| matches!(component, Component::ParentDir))
    {
        return Err(FileActionError::refused("请传入不包含 .. 的绝对路径。"));
    }
    Ok(())
}

pub fn reject_git(path: &Path) -> Result<(), FileActionError> {
    if path.components().any(|component| {
        matches!(component, Component::Normal(name) if name.to_string_lossy().eq_ignore_ascii_case(".git"))
    }) {
        return Err(FileActionError::refused("不能修改 .git 或其内部条目。"));
    }
    Ok(())
}

pub fn check_containment(root: &Path, parent: &Path) -> Result<(), FileActionError> {
    if !parent.starts_with(root) {
        return Err(FileActionError::refused("父目录不在授权范围内。"));
    }
    reject_git(parent)
}

pub fn entry_metadata(path: &Path) -> Result<Metadata, FileActionError> {
    fs::symlink_metadata(path).map_err(|error| FileActionError::io(path, error))
}

pub fn canonical_directory(path: &Path) -> Result<PathBuf, FileActionError> {
    let canonical = fs::canonicalize(path).map_err(|error| FileActionError::io(path, error))?;
    if !entry_metadata(&canonical)?.is_dir() {
        return Err(FileActionError::refused(format!("{} 不是目录。", path.display())));
    }
    Ok(canonical)
}

pub fn check_identity(path: &Path, expected: &Metadata) -> Result<(), FileActionError> {
    if !same_entry(&entry_metadata(path)?, expected) {
        return Err(FileActionError::refused(format!(
            "{} 已改变，请取消后重新选择。",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
pub fn same_entry(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino() && left.file_type() == right.file_type()
}

#[cfg(windows)]
pub fn same_entry(left: &Metadata, right: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
        && left.file_type() == right.file_type()
}

#[cfg(not(any(unix, windows)))]
pub fn same_entry(_: &Metadata, _: &Metadata) -> bool {
    false
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn check_file_identity(
    file: &File,
    path: &Path,
    expected: &Metadata,
) -> Result<(), FileActionError> {
    let metadata = file.metadata().map_err(|error| FileActionError::io(path, error))?;
    if !same_entry(&metadata, expected) {
        return Err(changed_entry(path));
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn open_directory_at(
    parent: &File,
    name: &OsStr,
    path: &Path,
) -> Result<File, FileActionError> {
    fd_fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|error| FileActionError::io(path, error.into()))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn stat_at(parent: &File, name: &OsStr, path: &Path) -> Result<Stat, FileActionError> {
    fd_fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| FileActionError::io(path, error.into()))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn stat_matches_metadata(stat: &Stat, expected: &Metadata) -> bool {
    #[cfg(target_os = "macos")]
    let device = stat.st_dev as u64;
    #[cfg(target_os = "linux")]
    let device = stat.st_dev;
    #[cfg(target_os = "macos")]
    let mode = u32::from(stat.st_mode);
    #[cfg(target_os = "linux")]
    let mode = stat.st_mode;
    device == expected.dev()
        && stat.st_ino == expected.ino()
        && (mode & 0o170000) == (expected.mode() & 0o170000)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn same_stat(left: &Stat, right: &Stat) -> bool {
    left.st_dev == right.st_dev
        && left.st_ino == right.st_ino
        && FileType::from_raw_mode(left.st_mode) == FileType::from_raw_mode(right.st_mode)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn changed_entry(path: &Path) -> FileActionError {
    FileActionError::refused(format!("{} 已改变，请取消后重新选择。", path.display()))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn rename_without_replacement(
    parent_file: &File,
    from: &Path,
    to: &Path,
    expected: &Metadata,
) -> Result<(), FileActionError> {
    let parent = from.parent().ok_or_else(|| FileActionError::refused("路径没有父目录。"))?;
    if to.parent() != Some(parent) {
        return Err(FileActionError::refused("重命名只能在原父目录中进行。"));
    }
    let from_name = from.file_name().ok_or_else(|| FileActionError::refused("原名称无效。"))?;
    let to_name = to.file_name().ok_or_else(|| FileActionError::refused("新名称无效。"))?;
    if !stat_matches_metadata(&stat_at(parent_file, from_name, from)?, expected) {
        return Err(changed_entry(from));
    }
    fd_fs::renameat_with(parent_file, from_name, parent_file, to_name, RenameFlags::NOREPLACE)
        .map_err(|error| FileActionError::io(to, error.into()))
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn directory_names(file: &File, path: &Path) -> Result<Vec<OsString>, FileActionError> {
    let directory =
        Dir::read_from(file).map_err(|error| FileActionError::io(path, error.into()))?;
    let mut names = Vec::new();
    for entry in directory {
        let entry = entry.map_err(|error| FileActionError::io(path, error.into()))?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        if bytes.eq_ignore_ascii_case(b".git") {
            return Err(FileActionError::refused(format!(
                "{} 含有 .git，不能永久删除此目录。",
                path.display()
            )));
        }
        if bytes.is_empty() || bytes.contains(&b'/') {
            return Err(FileActionError::refused("目录条目名称无效。"));
        }
        names.push(OsStr::from_bytes(bytes).to_owned());
    }
    names.sort_unstable();
    Ok(names)
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
struct DeleteEntry {
    name: OsString,
    stat: Stat,
    directory: Option<DeleteDirectory>,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl DeleteEntry {
    fn new(
        parent: &File,
        name: OsString,
        stat: Stat,
        path: &Path,
        root: &Metadata,
    ) -> Result<Self, FileActionError> {
        reject_git(path)?;
        if stat_matches_metadata(&stat, root) {
            return Err(FileActionError::refused("不能永久删除项目根目录。"));
        }
        let directory = if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            let file = open_directory_at(parent, &name, path)?;
            let opened =
                fd_fs::fstat(&file).map_err(|error| FileActionError::io(path, error.into()))?;
            if !same_stat(&opened, &stat) {
                return Err(changed_entry(path));
            }
            Some(DeleteDirectory::new(file, path, root)?)
        } else {
            None
        };
        Ok(Self { name, stat, directory })
    }

    fn check(&self, parent: &File, path: &Path) -> Result<(), FileActionError> {
        if !same_stat(&stat_at(parent, &self.name, path)?, &self.stat) {
            return Err(changed_entry(path));
        }
        if let Some(directory) = &self.directory {
            let opened = fd_fs::fstat(&directory.file)
                .map_err(|error| FileActionError::io(path, error.into()))?;
            if !same_stat(&opened, &self.stat) {
                return Err(changed_entry(path));
            }
        }
        Ok(())
    }

    fn preflight(&self, parent: &File, path: &Path) -> Result<(), FileActionError> {
        self.check(parent, path)?;
        if let Some(directory) = &self.directory {
            let names = directory_names(&directory.file, path)?;
            if names.len() != directory.entries.len()
                || names.iter().zip(&directory.entries).any(|(name, entry)| name != &entry.name)
            {
                return Err(changed_entry(path));
            }
            for entry in &directory.entries {
                entry.preflight(&directory.file, &path.join(&entry.name))?;
            }
        }
        Ok(())
    }

    fn remove(
        &self,
        parent: &File,
        path: &Path,
        removed: &mut bool,
    ) -> Result<(), FileActionError> {
        self.check(parent, path)?;
        let flags = if let Some(directory) = &self.directory {
            self.preflight(parent, path)?;
            for entry in &directory.entries {
                self.check(parent, path)?;
                entry.remove(&directory.file, &path.join(&entry.name), removed)?;
            }
            if !directory_names(&directory.file, path)?.is_empty() {
                return Err(changed_entry(path));
            }
            self.check(parent, path)?;
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        fd_fs::unlinkat(parent, &self.name, flags)
            .map_err(|error| FileActionError::io(path, error.into()))?;
        *removed = true;
        Ok(())
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
struct DeleteDirectory {
    file: File,
    entries: Vec<DeleteEntry>,
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
impl DeleteDirectory {
    fn new(file: File, path: &Path, root: &Metadata) -> Result<Self, FileActionError> {
        let names = directory_names(&file, path)?;
        let mut entries = Vec::with_capacity(names.len());
        for name in names {
            let child = path.join(&name);
            let stat = stat_at(&file, &name, &child)?;
            entries.push(DeleteEntry::new(&file, name, stat, &child, root)?);
        }
        Ok(Self { file, entries })
    }
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub fn delete_at(
    parent: &File,
    path: &Path,
    expected: &Metadata,
    root: &Metadata,
) -> Result<(), FileActionError> {
    let name = path.file_name().ok_or_else(|| FileActionError::refused("原名称无效。"))?;
    let stat = stat_at(parent, name, path)?;
    if !stat_matches_metadata(&stat, expected) {
        return Err(changed_entry(path));
    }
    let entry = DeleteEntry::new(parent, name.to_owned(), stat, path, root)?;
    entry.preflight(parent, path)?;
    let mut removed = false;
    entry.remove(parent, path, &mut removed).map_err(|error| {
        if removed { FileActionError::partial_delete(path, io::Error::other(error)) } else { error }
    })
}
