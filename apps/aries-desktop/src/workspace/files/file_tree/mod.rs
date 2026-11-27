use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use std::{fs, io};

use gpui_kit::assets::IconName;
use gpui_kit::base::GlobalState;
use gpui_kit::base::actions::{Confirm, SelectDown, SelectLeft, SelectRight, SelectUp};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState, MoveDown, MoveUp, Search};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::tree::{Tree, TreeEvent, TreeItem, TreeState};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;
use ignore::WalkBuilder;
use thiserror::Error;

use crate::workspace::files::OpenFile;
use crate::workspace::files::file_actions::{FileActionEvent, FileActions, FileCommand};
use crate::workspace::files::git_status::TreeGitStatus;

mod actions;
mod render;
mod reveal;
mod search;
mod tree;
mod watch;

const SEARCH_RESULT_LIMIT: usize = 200;
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(150);

gpui_kit::actions!(
    file_tree,
    [ConfirmEntry, PreviousFocus, FindEntry, RenameEntry, DeleteEntry, CopyRelativePath]
);

impl FindEntry {
    pub fn new() -> Self {
        Self
    }
}

impl RenameEntry {
    pub fn new() -> Self {
        Self
    }
}

impl DeleteEntry {
    pub fn new() -> Self {
        Self
    }
}

impl CopyRelativePath {
    pub fn new() -> Self {
        Self
    }
}

impl PreviousFocus {
    pub fn new() -> Self {
        Self
    }
}

impl ConfirmEntry {
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Error)]
enum FileTreeError {
    #[error("无法读取 {path}：{source}。请检查权限后刷新。")]
    Read { path: PathBuf, source: io::Error },
    #[error("无法遍历 {path}：{source}。请检查目录权限和忽略规则后刷新。")]
    Walk { path: PathBuf, source: ignore::Error },
    #[error("无法展开 {0}：不是目录或是符号链接目录。")]
    NotDirectory(PathBuf),
    #[error("无法定位 {0}：路径不在当前项目内。")]
    OutsideRoot(PathBuf),
    #[error("无法定位 {0}：文件不存在或已被忽略规则隐藏。")]
    Missing(PathBuf),
    #[error("无法定位：目录 {0} 读取失败，请刷新后重试。")]
    RevealBlocked(PathBuf),
}

impl FileTreeError {
    fn read(path: &Path, source: io::Error) -> Self {
        Self::Read { path: path.to_owned(), source }
    }

    fn walk(path: &Path, source: ignore::Error) -> Self {
        Self::Walk { path: path.to_owned(), source }
    }

    fn not_directory(path: &Path) -> Self {
        Self::NotDirectory(path.to_owned())
    }

    fn outside_root(path: &Path) -> Self {
        Self::OutsideRoot(path.to_owned())
    }

    fn missing(path: &Path) -> Self {
        Self::Missing(path.to_owned())
    }

    fn reveal_blocked(path: &Path) -> Self {
        Self::RevealBlocked(path.to_owned())
    }
}

#[derive(Clone)]
pub struct DiskEntry {
    path: PathBuf,
    directory: bool,
    symlink: bool,
}

impl DiskEntry {
    fn new(path: PathBuf, directory: bool, symlink: bool) -> Self {
        Self { path, directory, symlink }
    }

    fn kind(&self) -> &'static str {
        if self.symlink {
            "符号链接（不展开）"
        } else if self.directory {
            "文件夹"
        } else {
            "文件"
        }
    }

    fn label(&self) -> String {
        self.path.file_name().unwrap_or(self.path.as_os_str()).to_string_lossy().into_owned()
    }

    fn icon(&self) -> IconName {
        if self.symlink {
            return IconName::Link;
        }
        let name = self.path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
        if self.directory {
            return match name {
                "src" | "source" | "sources" => IconName::FolderCode,
                "test" | "tests" | "__tests__" => IconName::FlaskConical,
                _ => IconName::Folder,
            };
        }
        if name == ".env" || name.starts_with(".env.") {
            return IconName::FileKey;
        }
        if matches!(
            name,
            "Dockerfile"
                | "Containerfile"
                | "Cargo.lock"
                | "package-lock.json"
                | "yarn.lock"
                | "pnpm-lock.yaml"
        ) {
            return IconName::Package;
        }
        if matches!(name, "Makefile" | "Justfile" | "justfile") {
            return IconName::FileTerminal;
        }
        if matches!(name, ".gitignore" | ".gitattributes" | ".editorconfig") {
            return IconName::FileCog;
        }
        match self
            .path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "rs" | "java" | "kt" | "kts" | "c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "go"
            | "py" | "rb" | "swift" | "js" | "jsx" | "ts" | "tsx" | "vue" | "svelte" | "html"
            | "htm" | "css" | "scss" | "sass" | "less" => IconName::FileCode,
            "json" | "jsonc" | "json5" => IconName::FileBraces,
            "toml" | "yaml" | "yml" | "ini" | "conf" | "config" | "xml" | "properties" => {
                IconName::FileCog
            },
            "md" | "mdx" | "txt" | "rst" | "log" => IconName::FileText,
            "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd" => IconName::FileTerminal,
            "svg" | "png" | "jpg" | "jpeg" | "gif" | "webp" | "ico" | "bmp" | "avif" => {
                IconName::FileImage
            },
            "csv" | "tsv" | "xls" | "xlsx" => IconName::FileSpreadsheet,
            "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" | "jar" => IconName::FileArchive,
            "pem" | "key" | "crt" | "cer" => IconName::FileKey,
            "mp3" | "wav" | "ogg" | "flac" | "m4a" => IconName::Music,
            "mp4" | "mov" | "webm" | "mkv" => IconName::Film,
            _ => IconName::File,
        }
    }
}

enum DirectoryStatus {
    Unloaded,
    Loading,
    Loaded,
    Failed(FileTreeError),
}

impl DirectoryStatus {
    fn unloaded() -> Self {
        Self::Unloaded
    }

    fn loading() -> Self {
        Self::Loading
    }

    fn loaded() -> Self {
        Self::Loaded
    }

    fn failed(error: FileTreeError) -> Self {
        Self::Failed(error)
    }
}

struct DirectoryContents {
    entries: Vec<DiskEntry>,
    status: DirectoryStatus,
}

impl DirectoryContents {
    fn new() -> Self {
        Self { entries: Vec::new(), status: DirectoryStatus::unloaded() }
    }

    fn read(path: &Path, root: bool) -> Self {
        let mut contents = Self::new();
        let result = contents.read_entries(path, root, Some(1));
        contents.entries.sort_by(|left, right| {
            right.directory.cmp(&left.directory).then_with(|| {
                left.path
                    .file_name()
                    .cmp(&right.path.file_name())
                    .then_with(|| left.path.cmp(&right.path))
            })
        });
        contents.status = match result {
            Ok(()) => DirectoryStatus::loaded(),
            Err(error) => DirectoryStatus::failed(error),
        };
        contents
    }

    fn read_entries(
        &mut self,
        path: &Path,
        root: bool,
        max_depth: Option<usize>,
    ) -> Result<(), FileTreeError> {
        let metadata = if root { fs::metadata(path) } else { fs::symlink_metadata(path) }
            .map_err(|error| FileTreeError::read(path, error))?;
        if !metadata.is_dir() {
            return Err(FileTreeError::not_directory(path));
        }
        let mut builder = WalkBuilder::new(path);
        builder
            .hidden(false)
            .require_git(false)
            .follow_links(false)
            .max_depth(max_depth)
            .filter_entry(|entry| entry.depth() == 0 || entry.file_name() != ".git");
        let mut first_error = None;
        for result in builder.build() {
            let entry = match result {
                Ok(entry) => entry,
                Err(error) => {
                    first_error.get_or_insert_with(|| FileTreeError::walk(path, error));
                    continue;
                },
            };
            if let Some(error) = entry.error() {
                first_error.get_or_insert_with(|| FileTreeError::walk(entry.path(), error.clone()));
            }
            if entry.depth() == 0 {
                continue;
            }
            if let Some(kind) = entry.file_type() {
                self.entries.push(DiskEntry::new(
                    entry.into_path(),
                    kind.is_dir() && !kind.is_symlink(),
                    kind.is_symlink(),
                ));
            }
        }
        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

struct RevealRequest {
    path: PathBuf,
    target: Option<PathBuf>,
}

impl RevealRequest {
    fn new(path: PathBuf) -> Self {
        Self { path, target: None }
    }
}

struct FileSearch {
    input: Entity<InputState>,
    active: bool,
    query: String,
    index: Option<Arc<Vec<DiskEntry>>>,
    results: Vec<DiskEntry>,
    truncated: bool,
    error: Option<String>,
    saved_selection: Option<PathBuf>,
    revision: u64,
    index_task: Option<Task<()>>,
    filter_task: Option<Task<()>>,
}

impl FileSearch {
    fn new(input: Entity<InputState>) -> Self {
        Self {
            input,
            active: false,
            query: String::new(),
            index: None,
            results: Vec::new(),
            truncated: false,
            error: None,
            saved_selection: None,
            revision: 0,
            index_task: None,
            filter_task: None,
        }
    }

    fn has_query(&self) -> bool {
        self.active && !self.query.is_empty()
    }

    fn invalidate(&mut self) {
        self.revision += 1;
        self.index_task = None;
        self.filter_task = None;
        self.index = None;
        self.results.clear();
        self.truncated = false;
        self.error = None;
    }
}

#[derive(Clone)]
pub enum TreeCommand {
    File(FileCommand),
    Open(DiskEntry),
    Expand(PathBuf),
    Collapse,
    Refresh,
    Locate,
    Search,
    Copy(String),
}

impl TreeCommand {
    fn file(command: FileCommand) -> Self {
        Self::File(command)
    }
    fn open(entry: DiskEntry) -> Self {
        Self::Open(entry)
    }
    fn expand(path: PathBuf) -> Self {
        Self::Expand(path)
    }
    fn collapse() -> Self {
        Self::Collapse
    }
    fn refresh() -> Self {
        Self::Refresh
    }
    fn locate() -> Self {
        Self::Locate
    }
    fn search() -> Self {
        Self::Search
    }
    fn copy(text: String) -> Self {
        Self::Copy(text)
    }
}

pub struct FileTree {
    directory: PathBuf,
    root: PathBuf,
    state: Entity<TreeState>,
    actions: Entity<FileActions>,
    action_root: PathBuf,
    action_resolved_root: Option<PathBuf>,
    resolved_root: Option<PathBuf>,
    git_status: TreeGitStatus,
    git_error: Option<String>,
    git_task: Option<Task<()>>,
    watch_task: Option<Task<()>>,
    watch_error: Option<String>,
    search: FileSearch,
    current_file: Option<PathBuf>,
    _action_subscription: Subscription,
    _action_observation: Subscription,
    _input_subscription: Subscription,
    directories: HashMap<PathBuf, DirectoryContents>,
    paths: HashMap<SharedString, DiskEntry>,
    expanded: HashSet<PathBuf>,
    selected: Option<PathBuf>,
    pending_reveal: Option<RevealRequest>,
    reveal_error: Option<FileTreeError>,
    reveal_id: u64,
    reveal_task: Option<Task<()>>,
    generation: u64,
    tasks: HashMap<PathBuf, Task<()>>,
    focus: FocusHandle,
    _tree_subscription: Subscription,
    focus_subscription: Option<Subscription>,
    focus_out_subscription: Option<Subscription>,
}

impl EventEmitter<OpenFile> for FileTree {}
impl EventEmitter<FileActionEvent> for FileTree {}

fn map_root_path(path: &Path, root: &Path, resolved_root: Option<&Path>) -> PathBuf {
    if path.starts_with(root) {
        path.to_owned()
    } else if let Some(relative) =
        resolved_root.and_then(|resolved| path.strip_prefix(resolved).ok())
    {
        root.join(relative)
    } else {
        path.to_owned()
    }
}

fn replace_prefix(path: PathBuf, from: &Path, to: &Path) -> PathBuf {
    match path.strip_prefix(from) {
        Ok(relative) => to.join(relative),
        Err(_) => path,
    }
}

fn absolute_path(path: &Path) -> Result<PathBuf, FileTreeError> {
    std::path::absolute(path).map_err(|error| FileTreeError::read(path, error))
}

fn path_id(path: &Path) -> SharedString {
    let mut id = String::from("path:");
    for byte in path.as_os_str().as_encoded_bytes() {
        let _ = write!(id, "{byte:02x}");
    }
    id.into()
}
