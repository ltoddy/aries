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
    fn new() -> Self {
        Self
    }
}

impl ConfirmEntry {
    fn new() -> Self {
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
struct DiskEntry {
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
enum TreeCommand {
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

impl FileTree {
    pub fn new(directory: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let state = cx.new(|cx| TreeState::new(cx));
        let tree_subscription = cx.subscribe(&state, |this, _, event, cx| {
            let event = event.clone();
            let generation = this.generation;
            let view = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = view.update(cx, |this, cx| {
                    if this.generation == generation {
                        this.on_tree_event(&event, cx);
                    }
                });
            });
        });
        let actions = cx.new(FileActions::new);
        let action_subscription = cx.subscribe(&actions, |this, _, event, cx| {
            let event = event.clone();
            let root = this.action_root.clone();
            let resolved = this.action_resolved_root.clone();
            let view = cx.weak_entity();
            cx.defer(move |cx| {
                let _ = view.update(cx, |this, cx| {
                    if this.root == root {
                        this.on_file_action(event, resolved.as_deref(), cx);
                    }
                });
            });
        });
        let action_observation = cx.observe(&actions, |_, _, cx| cx.notify());
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("文件名或相对路径"));
        let input_subscription = cx.subscribe_in(&input, window, |_, _, event, window, cx| {
            if matches!(event, InputEvent::Focus | InputEvent::Blur) {
                cx.notify();
                return;
            }
            let change = matches!(event, InputEvent::Change);
            let enter = matches!(event, InputEvent::PressEnter { secondary: false, shift: false });
            if !change && !enter {
                return;
            }
            let view = cx.weak_entity();
            window.defer(cx, move |window, cx| {
                let _ = view.update(cx, |this, cx| {
                    if change {
                        this.query_changed(cx);
                    } else if this.search.active
                        && this.search_input_focused(window, cx)
                        && this.keyboard_available(window, cx)
                    {
                        this.activate_search_result(window, cx);
                    }
                });
            });
        });
        cx.bind_keys([
            KeyBinding::new("enter", ConfirmEntry::new(), Some("FileTreeContent")),
            KeyBinding::new("shift-tab", PreviousFocus::new(), Some("FileTree")),
            KeyBinding::new(
                if cfg!(target_os = "macos") { "cmd-f" } else { "ctrl-f" },
                FindEntry::new(),
                Some("FileTree"),
            ),
            KeyBinding::new("escape", Escape, Some("FileTree")),
            KeyBinding::new("f2", RenameEntry::new(), Some("FileTreeContent")),
            KeyBinding::new(
                if cfg!(target_os = "macos") { "cmd-backspace" } else { "ctrl-backspace" },
                DeleteEntry::new(),
                Some("FileTreeContent"),
            ),
            KeyBinding::new(
                if cfg!(target_os = "macos") { "cmd-shift-c" } else { "ctrl-shift-c" },
                CopyRelativePath::new(),
                Some("FileTreeContent"),
            ),
        ]);
        let view = cx.weak_entity();
        cx.defer(move |cx| {
            let _ = view.update(cx, |this, cx| {
                if this.generation == 0 {
                    this.load(cx);
                }
            });
        });
        let root = absolute_path(&directory).unwrap_or_else(|_| directory.clone());
        Self {
            directory,
            action_root: root.clone(),
            root,
            state,
            actions,
            action_resolved_root: None,
            resolved_root: None,
            git_status: TreeGitStatus::new(),
            git_error: None,
            git_task: None,
            search: FileSearch::new(input),
            current_file: None,
            _action_subscription: action_subscription,
            _action_observation: action_observation,
            _input_subscription: input_subscription,
            directories: HashMap::new(),
            paths: HashMap::new(),
            expanded: HashSet::new(),
            selected: None,
            pending_reveal: None,
            reveal_error: None,
            reveal_id: 0,
            reveal_task: None,
            generation: 0,
            tasks: HashMap::new(),
            focus: cx.focus_handle().tab_stop(true),
            _tree_subscription: tree_subscription,
            focus_subscription: None,
            focus_out_subscription: None,
        }
    }

    pub fn set_directory(&mut self, directory: PathBuf, cx: &mut Context<Self>) {
        if self.directory == directory {
            return;
        }
        self.root = absolute_path(&directory).unwrap_or_else(|_| directory.clone());
        self.directory = directory;
        self.resolved_root = None;
        self.git_status = TreeGitStatus::new();
        self.search.active = false;
        self.search.query.clear();
        self.search.saved_selection = None;
        self.current_file = None;
        self.directories.clear();
        self.paths.clear();
        self.expanded.clear();
        self.selected = None;
        self.cancel_reveal();
        self.reveal_error = None;
        self.state.update(cx, |state, cx| state.set_items(Vec::new(), cx));
        self.load(cx);
    }

    pub fn load(&mut self, cx: &mut Context<Self>) {
        self.remember_selection(cx);
        self.generation += 1;
        self.search.invalidate();
        self.load_git_status(cx);
        if self.search.has_query() {
            self.schedule_search(cx);
        }
        self.tasks.clear();
        self.reveal_task = None;
        if let Some(request) = self.pending_reveal.as_mut() {
            request.target = None;
        }
        self.reveal_error = None;
        for contents in self.directories.values_mut() {
            contents.status = DirectoryStatus::unloaded();
        }
        self.request_directory(self.root.clone(), cx);
        self.resolve_reveal(cx);
        self.rebuild(cx);
    }

    pub fn reveal(&mut self, path: &Path, cx: &mut Context<Self>) {
        if self.search.active {
            self.leave_search(cx);
        }
        self.remember_selection(cx);
        self.cancel_reveal();
        self.reveal_error = None;
        let path = if path.is_absolute() { path.to_owned() } else { self.root.join(path) };
        match absolute_path(&path) {
            Ok(path) => {
                self.pending_reveal = Some(RevealRequest::new(path));
                self.resolve_reveal(cx);
            },
            Err(error) => self.reveal_error = Some(error),
        }
        self.rebuild(cx);
    }

    fn cancel_reveal(&mut self) {
        self.reveal_id += 1;
        self.reveal_task = None;
        self.pending_reveal = None;
    }

    fn resolve_reveal(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.pending_reveal.as_ref() else {
            return;
        };
        let path = request.path.clone();
        let root = self.root.clone();
        let generation = self.generation;
        let reveal_id = self.reveal_id;
        self.reveal_task = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_spawn(async move {
                    let resolved_root = fs::canonicalize(&root)
                        .map_err(|error| FileTreeError::read(&root, error))?;
                    let target = fs::symlink_metadata(&path)
                        .and_then(|metadata| {
                            if path == root {
                                Ok(resolved_root.clone())
                            } else if metadata.file_type().is_symlink() {
                                let parent = path
                                    .parent()
                                    .ok_or_else(|| io::Error::other("路径没有父目录"))?;
                                let name = path
                                    .file_name()
                                    .ok_or_else(|| io::Error::other("路径没有名称"))?;
                                fs::canonicalize(parent).map(|parent| parent.join(name))
                            } else {
                                fs::canonicalize(&path)
                            }
                        })
                        .map_err(|error| {
                            if error.kind() == io::ErrorKind::NotFound {
                                FileTreeError::missing(&path)
                            } else {
                                FileTreeError::read(&path, error)
                            }
                        })?;
                    let relative = target
                        .strip_prefix(&resolved_root)
                        .map_err(|_| FileTreeError::outside_root(&path))?;
                    Ok::<_, FileTreeError>(root.join(relative))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation || this.reveal_id != reveal_id {
                    return;
                }
                this.reveal_task = None;
                this.remember_selection(cx);
                match result {
                    Ok(target) if target == this.root => {
                        this.selected = None;
                        this.pending_reveal = None;
                    },
                    Ok(target) => {
                        if let Some(request) = this.pending_reveal.as_mut() {
                            request.target = Some(target);
                        }
                    },
                    Err(error) => {
                        this.pending_reveal = None;
                        this.reveal_error = Some(error);
                    },
                }
                this.advance_reveal(cx);
                this.rebuild(cx);
            });
        }));
    }

    fn remember_selection(&mut self, cx: &App) {
        if let Some(entry) =
            self.state.read(cx).selected_item().and_then(|item| self.paths.get(&item.id))
        {
            self.selected = Some(entry.path.clone());
        }
    }

    fn request_directory(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let contents = self.directories.entry(path.clone()).or_insert_with(DirectoryContents::new);
        if !matches!(contents.status, DirectoryStatus::Unloaded) {
            return;
        }
        contents.status = DirectoryStatus::loading();
        let generation = self.generation;
        let root = path == self.root;
        let request_path = path.clone();
        let task = cx.spawn(async move |view, cx| {
            let read_path = request_path.clone();
            let contents =
                cx.background_spawn(async move { DirectoryContents::read(&read_path, root) }).await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.remember_selection(cx);
                this.tasks.remove(&request_path);
                let mut contents = contents;
                if matches!(
                    contents.status,
                    DirectoryStatus::Failed(
                        FileTreeError::Read { .. } | FileTreeError::Walk { .. }
                    )
                ) && contents.entries.is_empty()
                    && let Some(previous) = this.directories.get_mut(&request_path)
                {
                    contents.entries = std::mem::take(&mut previous.entries);
                }
                this.directories.insert(request_path, contents);
                this.request_expanded(cx);
                this.advance_reveal(cx);
                this.rebuild(cx);
            });
        });
        self.tasks.insert(path, task);
    }

    fn request_expanded(&mut self, cx: &mut Context<Self>) {
        let mut pending = vec![self.root.clone()];
        let mut requests = Vec::new();
        while let Some(path) = pending.pop() {
            match self.directories.get(&path) {
                None | Some(DirectoryContents { status: DirectoryStatus::Unloaded, .. }) => {
                    requests.push(path)
                },
                Some(contents)
                    if matches!(
                        contents.status,
                        DirectoryStatus::Loaded
                            | DirectoryStatus::Failed(
                                FileTreeError::Read { .. } | FileTreeError::Walk { .. }
                            )
                    ) =>
                {
                    pending.extend(
                        contents
                            .entries
                            .iter()
                            .filter(|entry| entry.directory && self.expanded.contains(&entry.path))
                            .map(|entry| entry.path.clone()),
                    );
                },
                Some(_) => {},
            }
        }
        for path in requests {
            self.request_directory(path, cx);
        }
    }

    fn advance_reveal(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.pending_reveal.as_ref().and_then(|request| request.target.clone())
        else {
            return;
        };
        let Ok(relative) = target.strip_prefix(&self.root) else {
            self.pending_reveal = None;
            self.reveal_error = Some(FileTreeError::outside_root(&target));
            return;
        };
        let mut parent = self.root.clone();
        let mut components = relative.components().peekable();
        while let Some(component) = components.next() {
            let contents = match self.directories.get(&parent) {
                None | Some(DirectoryContents { status: DirectoryStatus::Unloaded, .. }) => {
                    self.request_directory(parent, cx);
                    return;
                },
                Some(contents) if matches!(contents.status, DirectoryStatus::Loading) => return,
                Some(contents) => contents,
            };
            let path = parent.join(component);
            let Some(entry) = contents.entries.iter().find(|entry| entry.path == path) else {
                self.reveal_error =
                    Some(if matches!(contents.status, DirectoryStatus::Failed(_)) {
                        FileTreeError::reveal_blocked(&parent)
                    } else {
                        FileTreeError::missing(&target)
                    });
                self.pending_reveal = None;
                return;
            };
            if components.peek().is_none() {
                self.selected = Some(path.clone());
                let expanded = entry.directory && self.expanded.contains(&path);
                self.pending_reveal = None;
                if expanded {
                    self.request_expanded(cx);
                }
                self.rebuild(cx);
                let id = path_id(&target);
                self.state.update(cx, |state, cx| {
                    if let Some(ix) = state.index_of(&id) {
                        state.set_selected_index(Some(ix), cx);
                        state.scroll_to_item(ix, ScrollStrategy::Center);
                    }
                });
                return;
            }
            if !entry.directory {
                self.reveal_error = Some(FileTreeError::missing(&target));
                self.pending_reveal = None;
                return;
            }
            self.expanded.insert(path.clone());
            parent = path;
        }
    }

    fn on_tree_event(&mut self, event: &TreeEvent, cx: &mut Context<Self>) {
        if self.search.has_query() {
            return;
        }
        let (TreeEvent::Expanded(id) | TreeEvent::Collapsed(id)) = event;
        let Some(entry) = self.paths.get(id).filter(|entry| entry.directory) else {
            return;
        };
        let path = entry.path.clone();
        self.remember_selection(cx);
        self.cancel_reveal();
        self.set_expanded(path, matches!(event, TreeEvent::Expanded(_)), cx);
        self.rebuild(cx);
    }

    fn set_expanded(&mut self, path: PathBuf, expanded: bool, cx: &mut Context<Self>) {
        if expanded {
            self.expanded.insert(path.clone());
            if let Some(contents) = self.directories.get_mut(&path)
                && matches!(contents.status, DirectoryStatus::Failed(_))
            {
                contents.status = DirectoryStatus::unloaded();
            }
            self.request_expanded(cx);
        } else {
            self.expanded.remove(&path);
        }
    }

    fn confirm_entry(&mut self, _: &ConfirmEntry, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tree_focused(window, cx) || !self.keyboard_available(window, cx) {
            cx.propagate();
            return;
        }
        if self.search.has_query() {
            self.activate_search_result(window, cx);
            return;
        }
        let Some(item) = self.state.read(cx).selected_item() else {
            return;
        };
        let Some(entry) = self.paths.get(&item.id).cloned() else {
            return;
        };
        self.cancel_reveal();
        self.reveal_error = None;
        self.selected = Some(entry.path.clone());
        if entry.directory {
            let expanded = !self.expanded.contains(&entry.path);
            self.set_expanded(entry.path, expanded, cx);
            self.rebuild(cx);
        } else {
            cx.emit(OpenFile::new(self.directory.clone(), entry.path));
        }
    }

    fn clicked_entry(
        &mut self,
        id: &SharedString,
        generation: u64,
        reveal_id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if generation != self.generation
            || reveal_id != self.reveal_id
            || self.state.read(cx).selected_item().is_none_or(|item| &item.id != id)
        {
            return;
        }
        if self.search.has_query() {
            self.activate_search_result(window, cx);
            return;
        }
        self.cancel_reveal();
        self.reveal_error = None;
        if let Some(entry) = self.paths.get(id) {
            self.selected = Some(entry.path.clone());
            if !entry.directory {
                cx.emit(OpenFile::new(self.directory.clone(), entry.path.clone()));
            }
        }
        cx.notify();
    }

    fn select_enabled(&mut self, previous: bool, cx: &mut Context<Self>) {
        self.cancel_reveal();
        self.reveal_error = None;
        self.state.update(cx, |state, cx| {
            let mut len = 0;
            while state.entry(len).is_some() {
                len += 1;
            }
            let mut ix = state.selected_index().filter(|ix| *ix < len);
            for _ in 0..len {
                let next = if previous {
                    ix.and_then(|ix| ix.checked_sub(1)).unwrap_or(len - 1)
                } else {
                    ix.filter(|ix| ix + 1 < len).map_or(0, |ix| ix + 1)
                };
                if state.entry(next).is_some_and(|entry| !entry.is_disabled()) {
                    state.set_selected_index(Some(next), cx);
                    state.scroll_to_item(
                        next,
                        if previous { ScrollStrategy::Top } else { ScrollStrategy::Bottom },
                    );
                    return;
                }
                ix = Some(next);
            }
            state.set_selected_index(None, cx);
        });
        self.remember_selection(cx);
        cx.notify();
    }

    fn rebuild(&mut self, cx: &mut Context<Self>) {
        self.paths.clear();
        let items = if self.search.has_query() {
            let mut items: Vec<_> = self
                .search
                .results
                .iter()
                .map(|entry| {
                    let id = path_id(&entry.path);
                    self.paths.insert(id.clone(), entry.clone());
                    TreeItem::new(
                        id,
                        entry
                            .path
                            .strip_prefix(&self.root)
                            .unwrap_or(&entry.path)
                            .to_string_lossy()
                            .into_owned(),
                    )
                })
                .collect();
            if items.is_empty() {
                items.push(status_item(
                    &self.root,
                    if self.search.index_task.is_some() || self.search.filter_task.is_some() {
                        "正在搜索项目…".into()
                    } else if let Some(error) = &self.search.error {
                        error.clone()
                    } else {
                        "没有匹配项，试试其他文件名或相对路径".into()
                    },
                ));
            }
            items
        } else {
            build_items(&self.root, &self.directories, &self.expanded, &mut self.paths)
        };
        let selected_id = self.selected.as_deref().map(path_id);
        self.state.update(cx, |state, cx| {
            state.set_items(items, cx);
            let ix = selected_id.as_ref().and_then(|id| state.index_of(id));
            state.set_selected_index(ix, cx);
        });
        cx.notify();
    }

    fn load_git_status(&mut self, cx: &mut Context<Self>) {
        let root = self.root.clone();
        let generation = self.generation;
        self.git_error = None;
        self.git_task = Some(cx.spawn(async move |view, cx| {
            let (resolved, status) = cx
                .background_spawn(async move {
                    (fs::canonicalize(&root).ok(), TreeGitStatus::read(&root))
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.git_task = None;
                this.resolved_root = resolved;
                this.current_file = this
                    .current_file
                    .take()
                    .map(|path| map_root_path(&path, &this.root, this.resolved_root.as_deref()));
                match status {
                    Ok(status) => this.git_status = status,
                    Err(error) => {
                        this.git_status = TreeGitStatus::new();
                        this.git_error = Some(format!("Git 状态读取失败：{error}。刷新可重试。"));
                    },
                }
                cx.notify();
            });
        }));
    }

    fn on_file_action(
        &mut self,
        event: FileActionEvent,
        resolved_root: Option<&Path>,
        cx: &mut Context<Self>,
    ) {
        let map = |path: &Path| map_root_path(path, &self.root, resolved_root);
        let event = match event {
            FileActionEvent::Created(path) => FileActionEvent::created(map(&path)),
            FileActionEvent::Renamed { from, to } => FileActionEvent::renamed(map(&from), map(&to)),
            FileActionEvent::Deleted(path) => FileActionEvent::deleted(map(&path)),
            FileActionEvent::Refresh(path) => FileActionEvent::refresh(map(&path)),
        };
        self.remember_selection(cx);
        self.leave_search(cx);
        self.cancel_reveal();
        let target = match &event {
            FileActionEvent::Created(path) => Some(path.clone()),
            FileActionEvent::Refresh(_) => self.selected.clone(),
            FileActionEvent::Renamed { from, to } => {
                self.expanded =
                    self.expanded.drain().map(|path| replace_prefix(path, from, to)).collect();
                self.selected = self.selected.take().map(|path| replace_prefix(path, from, to));
                self.current_file =
                    self.current_file.take().map(|path| replace_prefix(path, from, to));
                self.selected
                    .clone()
                    .filter(|path| path.starts_with(to))
                    .or_else(|| Some(to.clone()))
            },
            FileActionEvent::Deleted(path) => {
                self.expanded.retain(|expanded| !expanded.starts_with(path));
                if self.current_file.as_ref().is_some_and(|current| current.starts_with(path)) {
                    self.current_file = None;
                }
                self.selected = self.selected.take().filter(|selected| !selected.starts_with(path));
                self.selected.clone().or_else(|| path.parent().map(Path::to_owned))
            },
        };
        self.state.update(cx, |state, cx| state.set_selected_index(None, cx));
        self.directories.clear();
        self.load(cx);
        if let Some(target) = target {
            self.reveal(&target, cx);
        }
        let owner = cx.weak_entity();
        let root = self.root.clone();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |this, cx| {
                if this.root == root {
                    cx.emit(event);
                }
            });
        });
    }

    pub fn cancel_actions(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.actions.update(cx, |actions, cx| actions.cancel(window, cx))
    }

    pub fn normalize_path(&self, path: &Path) -> PathBuf {
        let path = if path.is_absolute() { path.to_owned() } else { self.root.join(path) };
        map_root_path(&path, &self.root, self.resolved_root.as_deref())
    }

    pub fn track_current_file(&mut self, path: Option<PathBuf>, cx: &mut Context<Self>) {
        let path = path.map(|path| self.normalize_path(&path));
        if self.current_file != path {
            self.current_file = path;
            cx.notify();
        }
    }

    fn selected_entry(&self, cx: &App) -> Option<DiskEntry> {
        self.state.read(cx).selected_item().and_then(|item| self.paths.get(&item.id)).cloned()
    }

    fn tree_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus.contains_focused(window, cx) && !GlobalState::is_in_deferred_context(cx)
    }

    fn search_input_focused(&self, window: &Window, cx: &App) -> bool {
        self.search.input.focus_handle(cx).is_focused(window)
    }

    fn keyboard_available(&self, window: &mut Window, cx: &mut App) -> bool {
        (self.focus.contains_focused(window, cx) || self.search_input_focused(window, cx))
            && !window.has_active_dialog(cx)
            && !GlobalState::is_in_deferred_context(cx)
    }

    fn start_search(&mut self, value: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search.active {
            self.remember_selection(cx);
            self.cancel_reveal();
            self.search.saved_selection = self.selected.clone();
            self.search.active = true;
            self.search.query.clear();
            self.search.input.update(cx, |input, cx| input.set_value("", window, cx));
        }
        if let Some(value) = value {
            self.search.input.update(cx, |input, cx| input.set_value(value, window, cx));
            self.query_changed(cx);
        }
        self.search.input.update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }

    fn find_entry(&mut self, _: &FindEntry, window: &mut Window, cx: &mut Context<Self>) {
        if self.keyboard_available(window, cx) {
            self.start_search(None, window, cx);
        } else {
            cx.propagate();
        }
    }

    fn leave_search(&mut self, cx: &mut Context<Self>) {
        if !self.search.active {
            return;
        }
        self.search.active = false;
        self.search.query.clear();
        self.search.revision += 1;
        self.search.filter_task = None;
        self.selected = self.search.saved_selection.take();
        self.rebuild(cx);
    }

    fn escape_search(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.active && self.keyboard_available(window, cx) {
            self.leave_search(cx);
            self.focus(window, cx);
            cx.stop_propagation();
        } else {
            cx.propagate();
        }
    }

    fn typed_search(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = &event.keystroke;
        if !self.tree_focused(window, cx)
            || !self.keyboard_available(window, cx)
            || key.modifiers.platform
            || key.modifiers.control
            || key.modifiers.alt
            || key.modifiers.function
        {
            return;
        }
        let Some(text) = key
            .key_char
            .as_deref()
            .filter(|text| !text.is_empty() && text.chars().all(|ch| !ch.is_control()))
        else {
            return;
        };
        let mut value = if self.search.active {
            self.search.input.read(cx).value().to_string()
        } else {
            String::new()
        };
        value.push_str(text);
        self.start_search(Some(&value), window, cx);
        cx.stop_propagation();
    }

    fn query_changed(&mut self, cx: &mut Context<Self>) {
        if !self.search.active {
            return;
        }
        let query = self.search.input.read(cx).value().trim().to_lowercase();
        if query == self.search.query {
            return;
        }
        self.search.query = query;
        self.search.revision += 1;
        self.search.filter_task = None;
        self.search.results.clear();
        self.search.truncated = false;
        self.search.error = None;
        if self.search.query.is_empty() {
            self.selected = self.search.saved_selection.clone();
        }
        self.schedule_search(cx);
        self.rebuild(cx);
    }

    fn schedule_search(&mut self, cx: &mut Context<Self>) {
        if !self.search.has_query() {
            return;
        }
        let revision = self.search.revision;
        let generation = self.generation;
        self.search.filter_task = Some(cx.spawn(async move |view, cx| {
            cx.background_executor().timer(SEARCH_DEBOUNCE).await;
            let _ = view.update(cx, |this, cx| {
                if this.search.revision == revision
                    && this.generation == generation
                    && this.search.has_query()
                {
                    this.search.filter_task = None;
                    this.search_cached_index(cx);
                }
            });
        }));
    }

    fn search_cached_index(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.search.index.clone() else {
            if self.search.index_task.is_some() {
                return;
            }
            let root = self.root.clone();
            let generation = self.generation;
            self.search.error = None;
            self.search.index_task = Some(cx.spawn(async move |view, cx| {
                let result = cx.background_spawn(async move { read_search_index(&root) }).await;
                let _ = view.update(cx, |this, cx| {
                    if this.generation != generation {
                        return;
                    }
                    this.search.index_task = None;
                    match result {
                        Ok(index) => {
                            this.search.index = Some(Arc::new(index));
                            if this.search.has_query() {
                                this.search_cached_index(cx);
                            }
                        },
                        Err(error) => this.search.error = Some(error.to_string()),
                    }
                    this.rebuild(cx);
                });
            }));
            self.rebuild(cx);
            return;
        };
        let query = self.search.query.clone();
        let root = self.root.clone();
        let revision = self.search.revision;
        let generation = self.generation;
        self.search.filter_task = Some(cx.spawn(async move |view, cx| {
            let mut results = cx
                .background_spawn(async move {
                    index
                        .iter()
                        .filter(|entry| {
                            entry
                                .path
                                .strip_prefix(&root)
                                .unwrap_or(&entry.path)
                                .to_string_lossy()
                                .to_lowercase()
                                .contains(&query)
                        })
                        .take(SEARCH_RESULT_LIMIT + 1)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                if this.generation != generation
                    || this.search.revision != revision
                    || !this.search.has_query()
                {
                    return;
                }
                this.remember_selection(cx);
                this.search.filter_task = None;
                this.search.truncated = results.len() > SEARCH_RESULT_LIMIT;
                results.truncate(SEARCH_RESULT_LIMIT);
                if !results.iter().any(|entry| Some(&entry.path) == this.selected.as_ref()) {
                    this.selected = results.first().map(|entry| entry.path.clone());
                }
                this.search.results = results;
                this.rebuild(cx);
            });
        }));
    }

    fn activate_search_result(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.search.has_query() {
            return;
        }
        let Some(entry) = self.selected_entry(cx) else {
            return;
        };
        self.leave_search(cx);
        self.focus(window, cx);
        if entry.directory {
            self.expanded.insert(entry.path.clone());
        } else {
            cx.emit(OpenFile::new(self.directory.clone(), entry.path.clone()));
        }
        self.reveal(&entry.path, cx);
    }

    fn navigate_search(&mut self, previous: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.search.has_query()
            && self.search_input_focused(window, cx)
            && self.keyboard_available(window, cx)
        {
            self.select_enabled(previous, cx);
            cx.stop_propagation();
        }
    }

    fn collapse_all(&mut self, cx: &mut Context<Self>) {
        self.remember_selection(cx);
        self.cancel_reveal();
        self.expanded.clear();
        if self.search.has_query() {
            self.search.saved_selection = self
                .search
                .saved_selection
                .as_ref()
                .and_then(|path| path.strip_prefix(&self.root).ok())
                .and_then(|relative| relative.components().next())
                .map(|first| self.root.join(first));
        }
        if !self.search.has_query()
            && let Some(selected) = &self.selected
            && let Ok(relative) = selected.strip_prefix(&self.root)
            && let Some(first) = relative.components().next()
        {
            self.selected = Some(self.root.join(first));
        }
        self.rebuild(cx);
    }

    fn execute_command(
        &mut self,
        command: TreeCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.has_active_dialog(cx) {
            return;
        }
        match command {
            TreeCommand::File(command) => {
                if self.actions.read(cx).is_busy() || self.resolved_root.is_none() {
                    return;
                }
                self.action_root = self.root.clone();
                self.action_resolved_root = self.resolved_root.clone();
                self.actions.update(cx, |actions, cx| actions.execute(command, window, cx));
            },
            TreeCommand::Open(entry) => {
                self.leave_search(cx);
                self.focus(window, cx);
                cx.emit(OpenFile::new(self.directory.clone(), entry.path.clone()));
                self.reveal(&entry.path, cx);
            },
            TreeCommand::Expand(path) => {
                self.leave_search(cx);
                self.focus(window, cx);
                self.expanded.retain(|expanded| !expanded.starts_with(&path));
                self.expanded.insert(path.clone());
                self.reveal(&path, cx);
                self.request_expanded(cx);
            },
            TreeCommand::Collapse => self.collapse_all(cx),
            TreeCommand::Refresh => self.load(cx),
            TreeCommand::Locate => {
                if let Some(path) = self.current_file.clone() {
                    self.leave_search(cx);
                    self.reveal(&path, cx);
                    self.focus(window, cx);
                }
            },
            TreeCommand::Search => self.start_search(None, window, cx),
            TreeCommand::Copy(text) => cx.write_to_clipboard(ClipboardItem::new_string(text)),
        }
    }

    fn rename_entry(&mut self, _: &RenameEntry, window: &mut Window, cx: &mut Context<Self>) {
        if self.tree_focused(window, cx) && self.keyboard_available(window, cx) {
            if let Some(entry) = self.selected_entry(cx).filter(|entry| entry.path != self.root) {
                self.execute_command(
                    TreeCommand::file(FileCommand::rename(self.root.clone(), entry.path)),
                    window,
                    cx,
                );
            }
        } else {
            cx.propagate();
        }
    }

    fn delete_entry(&mut self, _: &DeleteEntry, window: &mut Window, cx: &mut Context<Self>) {
        if self.tree_focused(window, cx) && self.keyboard_available(window, cx) {
            if let Some(entry) = self.selected_entry(cx).filter(|entry| entry.path != self.root) {
                self.execute_command(
                    TreeCommand::file(FileCommand::delete(self.root.clone(), entry.path)),
                    window,
                    cx,
                );
            }
        } else {
            cx.propagate();
        }
    }

    fn copy_relative_path(
        &mut self,
        _: &CopyRelativePath,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.tree_focused(window, cx) && self.keyboard_available(window, cx) {
            if let Some(entry) = self.selected_entry(cx) {
                let path = entry
                    .path
                    .strip_prefix(&self.root)
                    .unwrap_or(&entry.path)
                    .to_string_lossy()
                    .into_owned();
                cx.write_to_clipboard(ClipboardItem::new_string(path));
            }
        } else {
            cx.propagate();
        }
    }

    fn menu(
        &self,
        mut menu: PopupMenu,
        entry: DiskEntry,
        header: bool,
        owner: WeakEntity<Self>,
        cx: &App,
    ) -> PopupMenu {
        let busy = self.actions.read(cx).is_busy() || self.resolved_root.is_none();
        let is_root = entry.path == self.root;
        let parent = if entry.directory {
            entry.path.clone()
        } else {
            entry.path.parent().unwrap_or(&self.root).to_owned()
        };
        menu = menu.label(entry.label());
        menu = self.menu_item(
            menu,
            if entry.directory { "展开选中文件夹（仅一层）" } else { "打开文件" },
            if entry.directory {
                TreeCommand::expand(entry.path.clone())
            } else {
                TreeCommand::open(entry.clone())
            },
            is_root,
            owner.clone(),
        );
        menu = menu.separator();
        for (label, command) in [
            ("新建文件…", FileCommand::create_file(self.root.clone(), parent.clone())),
            ("新建文件夹…", FileCommand::create_directory(self.root.clone(), parent.clone())),
        ] {
            menu = self.menu_item(menu, label, TreeCommand::file(command), busy, owner.clone());
        }
        menu = menu.separator();
        for (label, text) in [
            ("复制绝对路径", entry.path.to_string_lossy().into_owned()),
            (
                "复制相对路径",
                if is_root {
                    ".".into()
                } else {
                    entry
                        .path
                        .strip_prefix(&self.root)
                        .unwrap_or(&entry.path)
                        .to_string_lossy()
                        .into_owned()
                },
            ),
            ("复制名称", entry.label()),
        ] {
            menu = self.menu_item(menu, label, TreeCommand::copy(text), false, owner.clone());
        }
        menu = menu.separator();
        menu = self.menu_item(
            menu,
            "重命名…（F2）",
            TreeCommand::file(FileCommand::rename(self.root.clone(), entry.path.clone())),
            busy || is_root,
            owner.clone(),
        );
        menu = self.menu_item(
            menu,
            "永久删除…",
            TreeCommand::file(FileCommand::delete(self.root.clone(), entry.path)),
            busy || is_root,
            owner.clone(),
        );
        if header {
            menu = menu.separator().label("项目");
            for (label, command, disabled) in [
                (
                    "在项目根目录新建文件…",
                    TreeCommand::file(FileCommand::create_file(
                        self.root.clone(),
                        self.root.clone(),
                    )),
                    busy,
                ),
                (
                    "在项目根目录新建文件夹…",
                    TreeCommand::file(FileCommand::create_directory(
                        self.root.clone(),
                        self.root.clone(),
                    )),
                    busy,
                ),
            ] {
                menu = self.menu_item(menu, label, command, disabled, owner.clone());
            }
        }
        menu
    }

    fn menu_item(
        &self,
        menu: PopupMenu,
        label: &str,
        command: TreeCommand,
        disabled: bool,
        owner: WeakEntity<Self>,
    ) -> PopupMenu {
        let root = self.root.clone();
        let generation = self.generation;
        menu.item(PopupMenuItem::new(label.to_owned()).disabled(disabled).on_click(
            move |_, window, cx| {
                let owner = owner.clone();
                let root = root.clone();
                let command = command.clone();
                window.defer(cx, move |window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.root == root && this.generation == generation {
                            this.execute_command(command, window, cx);
                        }
                    });
                });
            },
        ))
    }

    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.initialize_focus(window, cx);
        self.focus.focus(window, cx);
    }

    fn previous_focus(&mut self, _: &PreviousFocus, window: &mut Window, cx: &mut Context<Self>) {
        if !self.tree_focused(window, cx) || !self.keyboard_available(window, cx) {
            cx.propagate();
            return;
        }
        if !self.focus.is_focused(window) {
            self.focus.focus(window, cx);
        }
        window.focus_prev(cx);
    }

    fn initialize_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.focus_subscription.is_some() {
            return;
        }
        self.focus_out_subscription =
            Some(cx.on_focus_out(&self.focus, window, |_, _, _, cx| cx.notify()));
        self.focus_subscription = Some(cx.on_focus(&self.focus, window, |this, window, cx| {
            this.state.update(cx, |state, cx| {
                state.focus(window, cx);
                if state.selected_item().is_none() {
                    let mut ix = 0;
                    while let Some(entry) = state.entry(ix) {
                        if !entry.is_disabled() {
                            state.set_selected_index(Some(ix), cx);
                            break;
                        }
                        ix += 1;
                    }
                }
            });
            cx.notify();
        }));
    }
}

fn read_search_index(root: &Path) -> Result<Vec<DiskEntry>, FileTreeError> {
    let mut contents = DirectoryContents::new();
    contents.read_entries(root, true, None)?;
    contents.entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(contents.entries)
}

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

fn status_item(path: &Path, label: String) -> TreeItem {
    TreeItem::new(format!("status:{}", path_id(path)), label).disabled(true)
}

fn build_items(
    path: &Path,
    directories: &HashMap<PathBuf, DirectoryContents>,
    expanded: &HashSet<PathBuf>,
    paths: &mut HashMap<SharedString, DiskEntry>,
) -> Vec<TreeItem> {
    let Some(contents) = directories.get(path) else {
        return vec![status_item(path, "展开以读取目录".into())];
    };
    let mut items: Vec<TreeItem> = contents
        .entries
        .iter()
        .map(|entry| {
            let id = path_id(&entry.path);
            paths.insert(id.clone(), entry.clone());
            let item = TreeItem::new(id, entry.label());
            if entry.directory {
                let is_expanded = expanded.contains(&entry.path);
                let children = if is_expanded {
                    build_items(&entry.path, directories, expanded, paths)
                } else {
                    vec![status_item(&entry.path, "展开以读取目录".into())]
                };
                item.children(children).expanded(is_expanded)
            } else {
                item
            }
        })
        .collect();
    let status = match &contents.status {
        DirectoryStatus::Unloaded => Some("等待加载目录".into()),
        DirectoryStatus::Loading => Some("正在读取目录…".into()),
        DirectoryStatus::Loaded if items.is_empty() => {
            Some("没有可显示的条目（已过滤忽略项）".into())
        },
        DirectoryStatus::Loaded => None,
        DirectoryStatus::Failed(error) => Some(error.to_string()),
    };
    if let Some(status) = status {
        items.push(status_item(path, status));
    }
    items
}

impl Focusable for FileTree {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for FileTree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.initialize_focus(window, cx);
        let view = cx.weak_entity();
        let header_view = view.clone();
        let tree_state = self.state.clone();
        let generation = self.generation;
        let root = self.root.clone();
        let loading = !self.tasks.is_empty()
            || self.reveal_task.is_some()
            || self.git_task.is_some()
            || self.search.index_task.is_some()
            || self.search.filter_task.is_some()
            || self.actions.read(cx).is_busy();
        let focused = self.tree_focused(window, cx);
        let error = self
            .reveal_error
            .as_ref()
            .map(ToString::to_string)
            .or_else(|| self.actions.read(cx).error().map(str::to_owned))
            .or_else(|| self.git_error.clone());
        div()
            .size_full().min_w_0().min_h_0().flex().flex_col()
            .key_context("FileTree")
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(Self::find_entry))
            .capture_action(cx.listener(Self::escape_search))
            .child(
                div().flex().items_center().flex_shrink_0().gap_1().px_3().py_1()
                    .child(Icon::new(IconName::FolderTree).small().text_color(cx.theme().muted_foreground))
                    .child(
                        div().id("file-tree-directory").flex_1().min_w_0().truncate().font_weight(FontWeight::SEMIBOLD)
                            .child(self.directory.file_name().unwrap_or(self.directory.as_os_str()).to_string_lossy().into_owned())
                            .tooltip({
                                let directory = self.root.display().to_string();
                                move |window, cx| Tooltip::new(directory.clone()).build(window, cx)
                            }),
                    )
                    .when(loading, |header| header.child(Spinner::new().small()))
                    .child(Button::new("search-project-files").ghost().small().icon(Icon::new(IconName::Search).size(rems(0.9375)))
                        .accessibility_label("搜索项目文件").tooltip_with_action("搜索项目文件", &FindEntry::new(), None)
                        .on_click(cx.listener(|this, _, window, cx| this.execute_command(TreeCommand::search(), window, cx))))
                    .child(Button::new("locate-current-file").ghost().small().icon(Icon::new(IconName::LocateFixed).size(rems(0.9375)))
                        .accessibility_label("定位当前文件").tooltip("定位当前文件").disabled(self.current_file.is_none())
                        .on_click(cx.listener(|this, _, window, cx| this.execute_command(TreeCommand::locate(), window, cx))))
                    .child(Button::new("collapse-file-tree").ghost().small().icon(Icon::new(IconName::ListCollapse).size(rems(0.9375)))
                        .accessibility_label("全部折叠").tooltip("全部折叠").disabled(self.expanded.is_empty())
                        .on_click(cx.listener(|this, _, window, cx| this.execute_command(TreeCommand::collapse(), window, cx))))
                    .child(Button::new("refresh-file-tree").ghost().small().icon(Icon::new(IconName::RefreshCw).size(rems(0.9375)))
                        .accessibility_label("刷新目录和 Git 状态").tooltip("刷新目录和 Git 状态")
                        .on_click(cx.listener(|this, _, window, cx| this.execute_command(TreeCommand::refresh(), window, cx))))
                    .child(Button::new("file-tree-more").ghost().small().icon(Icon::new(IconName::Ellipsis).size(rems(0.9375)))
                        .accessibility_label("文件操作及项目菜单").tooltip("文件操作及项目菜单")
                        .dropdown_menu(move |menu, _, cx| {
                            let Some(view) = header_view.upgrade() else { return menu; };
                            let this = view.read(cx);
                            if this.root != root || this.generation != generation { return menu; }
                            let entry = this.selected_entry(cx).unwrap_or_else(|| DiskEntry::new(this.root.clone(), true, false));
                            this.menu(menu, entry, true, header_view.clone(), cx)
                        })),
            )
            .when(self.search.active, |tree| {
                tree.child(div().id("file-tree-search").flex_shrink_0().flex().flex_col().gap_1().px_3().py_2()
                    .capture_action(cx.listener(|this, _: &Search, window, cx| {
                        if this.search_input_focused(window, cx) && this.keyboard_available(window, cx) {
                            this.start_search(None, window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &MoveUp, window, cx| this.navigate_search(true, window, cx)))
                    .capture_action(cx.listener(|this, _: &MoveDown, window, cx| this.navigate_search(false, window, cx)))
                    .capture_action(cx.listener(|this, _: &SelectUp, window, cx| this.navigate_search(true, window, cx)))
                    .capture_action(cx.listener(|this, _: &SelectDown, window, cx| this.navigate_search(false, window, cx)))
                    .child(div().flex().items_center().gap_1()
                        .child(div().flex_1().min_w_0().child(Input::new(&self.search.input).small().aria_label("搜索项目文件名或相对路径")))
                        .child(Button::new("close-file-search").ghost().small().icon(IconName::X).accessibility_label("取消搜索").tooltip("取消搜索（Escape）")
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.leave_search(cx);
                                this.focus(window, cx);
                            }))))
                    .child(div().id("file-tree-search-scope").text_xs().text_color(cx.theme().muted_foreground).truncate()
                        .child("项目文件 · 不含忽略目录")
                        .tooltip(|window, cx| Tooltip::new("搜索整个项目的文件名和相对路径；遵循 ignore 规则，不包含忽略目录、.git 或链接目录内的条目。目录读取或规则解析失败会显示错误。").build(window, cx)))
                    .when(self.search.has_query(), |search| search.child(div().text_xs().text_color(cx.theme().muted_foreground)
                        .child(if self.search.truncated {
                            format!("仅显示前 {SEARCH_RESULT_LIMIT} 项，请缩小搜索范围")
                        } else if self.search.index_task.is_some() || self.search.filter_task.is_some() {
                            "正在搜索…".into()
                        } else { format!("{} 项 · ↑↓ 选择 · Enter 打开 · Escape 返回", self.search.results.len()) }))))
            })
            .child(
                div().id("file-tree-content").flex_1().min_w_0().min_h_0()
                    .key_context("FileTreeContent").track_focus(&self.focus)
                    .border_1().border_color(if focused { cx.theme().ring } else { cx.theme().background })
                    .on_key_down(cx.listener(Self::typed_search))
                    .capture_action(cx.listener(|this, _: &SelectLeft, window, cx| {
                        if GlobalState::is_in_deferred_context(cx) && !this.tree_focused(window, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &SelectRight, window, cx| {
                        if GlobalState::is_in_deferred_context(cx) && !this.tree_focused(window, cx) {
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &SelectUp, window, cx| {
                        if this.tree_focused(window, cx) && this.keyboard_available(window, cx) {
                            this.select_enabled(true, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &SelectDown, window, cx| {
                        if this.tree_focused(window, cx) && this.keyboard_available(window, cx) {
                            this.select_enabled(false, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(|this, _: &Confirm, window, cx| {
                        if this.tree_focused(window, cx) && this.keyboard_available(window, cx) {
                            this.confirm_entry(&ConfirmEntry::new(), window, cx);
                            cx.stop_propagation();
                        }
                    }))
                    .capture_action(cx.listener(Self::confirm_entry))
                    .on_action(cx.listener(Self::previous_focus))
                    .on_action(cx.listener(Self::rename_entry))
                    .on_action(cx.listener(Self::delete_entry))
                    .on_action(cx.listener(Self::copy_relative_path))
                    .child(Tree::new(&self.state, move |_, entry, _, _, cx| {
                        let id = entry.item().id.clone();
                        let label = entry.item().label.clone();
                        let data = view.upgrade().and_then(|view| view.read(cx).paths.get(&id).cloned());
                        let status = data.as_ref().and_then(|data| view.upgrade().and_then(|view| view.read(cx).git_status.status(&data.path)));
                        let color = status.map_or(cx.theme().foreground, |status| status.color(cx));
                        let tooltip = data.as_ref().map_or_else(|| label.to_string(), |data| {
                            format!("{} · {}\n{}", data.kind(), status.map_or("无 Git 变更状态", |status| status.label()), data.path.display())
                        });
                        let icon = if entry.is_disabled() { IconName::Info }
                            else {
                                let icon = data.as_ref().map_or(IconName::File, DiskEntry::icon);
                                if icon == IconName::Folder && entry.is_expanded() { IconName::FolderOpen } else { icon }
                            };
                        let menu_view = view.clone();
                        let select_view = view.clone();
                        let select_id = id.clone();
                        let menu_id = id.clone();
                        let click_view = view.clone();
                        let click_state = tree_state.clone();
                        ListItem::new(id.clone()).h_6().py_0().px_2().text_sm()
                            .accessibility_label(tooltip.clone())
                            .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                            .child(
                                div().id(SharedString::from(format!("file-tree-row:{id}")))
                                    .flex().items_center().gap_1().w_full().min_w_0().pl(rems(entry.depth() as f32))
                                    .on_mouse_down(MouseButton::Right, move |_, window, cx| {
                                        let view = select_view.clone();
                                        let id = select_id.clone();
                                        window.defer(cx, move |_, cx| {
                                            let _ = view.update(cx, |this, cx| {
                                                if this.generation != generation || !this.paths.contains_key(&id) { return; }
                                                this.cancel_reveal();
                                                this.state.update(cx, |state, cx| {
                                                    if let Some(ix) = state.index_of(&id) { state.set_selected_index(Some(ix), cx); }
                                                });
                                                this.remember_selection(cx);
                                                cx.notify();
                                            });
                                        });
                                    })
                                    .child(div().w_4().flex_shrink_0().flex().items_center().justify_center()
                                        .when(entry.is_folder(), |slot| slot.child(Icon::new(if entry.is_expanded() { IconName::ChevronDown } else { IconName::ChevronRight })
                                            .xsmall().text_color(cx.theme().muted_foreground))))
                                    .child(div().size_4().flex_shrink_0().flex().items_center().justify_center().child(Icon::new(icon).small()))
                                    .child(div().flex_1().min_w_0().truncate().text_color(color).child(label))
                                    .child(div().w_4().flex_shrink_0().text_center().text_color(color).child(status.map_or("", |status| status.marker())))
                                    .context_menu(move |menu, _, cx| {
                                        let Some(view) = menu_view.upgrade() else { return menu; };
                                        let this = view.read(cx);
                                        if this.generation != generation { return menu; }
                                        let Some(entry) = this.paths.get(&menu_id).cloned() else { return menu; };
                                        this.menu(menu, entry, false, menu_view.clone(), cx)
                                    }),
                            )
                            .on_click(move |_, window, cx| {
                                let Ok(Some(reveal_id)) = click_view.update(cx, |this, cx| {
                                    if this.generation != generation { return None; }
                                    this.cancel_reveal();
                                    this.reveal_error = None;
                                    cx.notify();
                                    Some(this.reveal_id)
                                }) else { return; };
                                click_state.update(cx, |state, cx| state.focus(window, cx));
                                let view = click_view.clone();
                                let id = id.clone();
                                window.defer(cx, move |window, cx| {
                                    let _ = view.update(cx, |this, cx| this.clicked_entry(&id, generation, reveal_id, window, cx));
                                });
                            })
                    })),
            )
            .when_some(error, |tree, error| tree.child(div().px_3().py_2().text_sm().text_color(cx.theme().danger).child(error)))
    }
}
