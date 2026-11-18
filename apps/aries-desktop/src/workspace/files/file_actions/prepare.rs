use std::fs::{File, Metadata};
#[cfg(any(target_os = "macos", target_os = "linux"))]
use std::path::Component;
use std::path::PathBuf;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use rustix::fs::{self as fd_fs, Mode, OFlags};

use crate::workspace::files::file_actions::fs::{
    canonical_directory, check_containment, check_file_identity, check_identity, delete_at,
    entry_metadata, open_directory_at, reject_git, rename_without_replacement, require_absolute,
    same_entry,
};
use crate::workspace::files::file_actions::{
    FileActionError, FileActionEvent, FileCommand, validate_basename,
};

#[derive(Clone)]
pub struct PreparedAction {
    command: FileCommand,
    root: PathBuf,
    root_metadata: Metadata,
    parent: PathBuf,
    parent_metadata: Metadata,
    entry_metadata: Option<Metadata>,
}

impl PreparedAction {
    pub fn new(command: FileCommand) -> Result<Self, FileActionError> {
        let (root, parent, path) = match &command {
            FileCommand::CreateFile { root, parent }
            | FileCommand::CreateDirectory { root, parent } => (root, parent.as_path(), None),
            FileCommand::Rename { root, path } | FileCommand::Delete { root, path } => {
                let parent =
                    path.parent().ok_or_else(|| FileActionError::refused("路径没有父目录。"))?;
                (root, parent, Some(path))
            },
        };
        require_absolute(root)?;
        reject_git(root)?;
        let root = canonical_directory(root)?;
        require_absolute(parent)?;
        reject_git(parent)?;
        let parent = canonical_directory(parent)?;
        check_containment(&root, &parent)?;
        reject_git(&root)?;
        if root.parent().is_none() {
            return Err(FileActionError::refused("不能修改文件系统根目录。"));
        }
        let root_metadata = entry_metadata(&root)?;
        let (path, metadata) = if let Some(path) = path {
            require_absolute(path)?;
            reject_git(path)?;
            let name = path
                .file_name()
                .ok_or_else(|| FileActionError::refused("不能重命名或删除项目根目录、. 或 ..。"))?;
            if name == "." || name == ".." {
                return Err(FileActionError::refused("不能操作 . 或 ..。"));
            }
            let path = parent.join(name);
            if path == root {
                return Err(FileActionError::refused("不能重命名或删除项目根目录。"));
            }
            let metadata = entry_metadata(&path)?;
            if metadata.is_dir() && same_entry(&metadata, &root_metadata) {
                return Err(FileActionError::refused("不能重命名或删除项目根目录。"));
            }
            (path, Some(metadata))
        } else {
            (parent.clone(), None)
        };
        reject_git(&parent)?;
        reject_git(&path)?;
        let parent_metadata = entry_metadata(&parent)?;
        let command = match command {
            FileCommand::CreateFile { .. } => {
                FileCommand::create_file(root.clone(), parent.clone())
            },
            FileCommand::CreateDirectory { .. } => {
                FileCommand::create_directory(root.clone(), parent.clone())
            },
            FileCommand::Rename { .. } => FileCommand::rename(root.clone(), path),
            FileCommand::Delete { .. } => FileCommand::delete(root.clone(), path),
        };
        Ok(Self { command, root, root_metadata, parent, parent_metadata, entry_metadata: metadata })
    }

    pub fn command(&self) -> &FileCommand {
        &self.command
    }

    pub fn entry_metadata(&self) -> Option<&Metadata> {
        self.entry_metadata.as_ref()
    }

    fn revalidate(&self) -> Result<(), FileActionError> {
        let root = canonical_directory(&self.root)?;
        let parent = canonical_directory(&self.parent)?;
        if root != self.root || parent != self.parent {
            return Err(FileActionError::refused("目录位置已改变，请取消后重新选择。"));
        }
        check_containment(&root, &parent)?;
        check_identity(&root, &self.root_metadata)?;
        check_identity(&parent, &self.parent_metadata)?;
        if let Some(metadata) = &self.entry_metadata {
            check_identity(self.command.path(), metadata)?;
        }
        Ok(())
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn open_parent(&self) -> Result<File, FileActionError> {
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let root = fd_fs::open(&self.root, flags, Mode::empty())
            .map(File::from)
            .map_err(|error| FileActionError::io(&self.root, error.into()))?;
        check_file_identity(&root, &self.root, &self.root_metadata)?;
        let relative = self
            .parent
            .strip_prefix(&self.root)
            .map_err(|_| FileActionError::refused("父目录不在授权范围内。"))?;
        let mut directory = root;
        let mut path = self.root.clone();
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(FileActionError::refused("父目录路径无效。"));
            };
            path.push(name);
            directory = open_directory_at(&directory, name, &path)?;
        }
        check_file_identity(&directory, &self.parent, &self.parent_metadata)?;
        Ok(directory)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    pub fn execute(&self, _: Option<&str>) -> Result<FileActionEvent, FileActionError> {
        self.revalidate()?;
        Err(FileActionError::unsupported("此平台没有可用的安全目录句柄操作接口，原条目保持不变。"))
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub fn execute(&self, name: Option<&str>) -> Result<FileActionEvent, FileActionError> {
        self.revalidate()?;
        let parent_file = self.open_parent()?;
        match &self.command {
            FileCommand::CreateFile { parent, .. }
            | FileCommand::CreateDirectory { parent, .. } => {
                let name = name.ok_or_else(|| FileActionError::invalid_name("请输入名称。"))?;
                validate_basename(name)?;
                let path = parent.join(name);
                if matches!(self.command, FileCommand::CreateFile { .. }) {
                    let file = fd_fs::openat(
                        &parent_file,
                        name,
                        OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::CLOEXEC,
                        Mode::from_raw_mode(0o666),
                    )
                    .map_err(|error| FileActionError::io(&path, error.into()))?;
                    drop(file);
                } else {
                    fd_fs::mkdirat(&parent_file, name, Mode::from_raw_mode(0o777))
                        .map_err(|error| FileActionError::io(&path, error.into()))?;
                }
                Ok(FileActionEvent::created(path))
            },
            FileCommand::Rename { path, .. } => {
                let name = name.ok_or_else(|| FileActionError::invalid_name("请输入名称。"))?;
                validate_basename(name)?;
                let to = self.parent.join(name);
                if &to == path {
                    return Err(FileActionError::invalid_name("新名称与原名称相同。"));
                }
                let metadata = self.entry_metadata.as_ref().ok_or_else(|| {
                    FileActionError::refused("缺少原条目信息，请取消后重新选择。")
                })?;
                rename_without_replacement(&parent_file, path, &to, metadata)?;
                Ok(FileActionEvent::renamed(path.clone(), to))
            },
            FileCommand::Delete { path, .. } => {
                let expected = self.entry_metadata.as_ref().ok_or_else(|| {
                    FileActionError::refused("缺少原条目信息，请取消后重新选择。")
                })?;
                delete_at(&parent_file, path, expected, &self.root_metadata)?;
                Ok(FileActionEvent::deleted(path.clone()))
            },
        }
    }
}
