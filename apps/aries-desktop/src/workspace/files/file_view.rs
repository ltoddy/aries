use std::fs::{self, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::str::Utf8Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use gpui_kit::component::button::Button;
use gpui_kit::component::input::{
    Editor, EditorState, Escape, InputEvent, RopeExt, Search, TabSize,
};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use thiserror::Error;
use url::Url;

const FILE_VIEW_CONTEXT: &str = "FileView";
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const AUTO_SAVE_DELAY: Duration = Duration::from_millis(500);
const UTF8_BOM: &[u8] = &[0xef, 0xbb, 0xbf];
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

actions!(file_view, [FindFile, CloseFile, CopyFile, NextMatch, PreviousMatch, SaveFile]);

impl SaveFile {
    pub fn new() -> Self {
        Self
    }
}

impl FindFile {
    pub fn new() -> Self {
        Self
    }
}

impl CloseFile {
    pub fn new() -> Self {
        Self
    }
}

impl CopyFile {
    pub fn new() -> Self {
        Self
    }
}

impl NextMatch {
    pub fn new() -> Self {
        Self
    }
}

impl PreviousMatch {
    pub fn new() -> Self {
        Self
    }
}

pub fn init(cx: &mut App) {
    let (find, copy, next, previous) = if cfg!(target_os = "macos") {
        ("cmd-f", "cmd-shift-c", "cmd-g", "cmd-shift-g")
    } else {
        ("ctrl-f", "ctrl-shift-c", "ctrl-g", "ctrl-shift-g")
    };
    cx.bind_keys([
        KeyBinding::new(find, FindFile::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(copy, CopyFile::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(
            if cfg!(target_os = "macos") { "cmd-s" } else { "ctrl-s" },
            SaveFile::new(),
            Some(FILE_VIEW_CONTEXT),
        ),
        KeyBinding::new(next, NextMatch::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(previous, PreviousMatch::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new("f3", NextMatch::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new("shift-f3", PreviousMatch::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new("escape", Escape, Some(FILE_VIEW_CONTEXT)),
    ]);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenFile {
    directory: PathBuf,
    path: PathBuf,
    line: Option<NonZeroUsize>,
}

impl OpenFile {
    pub fn new(directory: PathBuf, path: PathBuf) -> Self {
        let directory = std::path::absolute(&directory).unwrap_or(directory);
        let path = if path.is_absolute() { path } else { directory.join(path) };
        Self { directory, path, line: None }
    }

    pub fn from_location(directory: PathBuf, location: &str) -> Option<Self> {
        let location = location.trim();
        if location.is_empty() || location.contains('\0') {
            return None;
        }
        let (location, anchor_line) = match location.rsplit_once("#L") {
            Some((path, line)) => (path, Some(line.parse::<NonZeroUsize>().ok()?)),
            None => (location, None),
        };
        let directory = std::path::absolute(&directory).unwrap_or(directory);
        let (path, suffix_line) = match Url::parse(location) {
            Ok(url) if url.scheme() == "file" => {
                if url.query().is_some() || url.fragment().is_some() {
                    return None;
                }
                (url.to_file_path().ok()?, None)
            },
            _ => {
                let (location, suffix_line) = split_line_suffix(location)?;
                if location.is_empty() {
                    return None;
                }
                let windows_drive = location.as_bytes().get(1) == Some(&b':')
                    && location.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
                    && location.as_bytes().get(2).is_some_and(|byte| matches!(byte, b'/' | b'\\'));
                if !windows_drive
                    && (Url::parse(location).is_ok()
                        || location.contains("://")
                        || location.starts_with("//"))
                {
                    return None;
                }
                let path = if !windows_drive && location.contains('%') {
                    let base = Url::from_directory_path(&directory).ok()?;
                    let url = base.join(location).ok()?;
                    if url.scheme() != "file" || url.query().is_some() || url.fragment().is_some() {
                        return None;
                    }
                    url.to_file_path().ok()?
                } else {
                    PathBuf::from(location)
                };
                (path, suffix_line)
            },
        };
        if path.as_os_str().is_empty() || path.as_os_str().as_encoded_bytes().contains(&0) {
            return None;
        }
        let target = Self::new(directory, path);
        Some(match anchor_line.or(suffix_line) {
            Some(line) => target.with_line(line),
            None => target,
        })
    }

    pub fn with_line(mut self, line: NonZeroUsize) -> Self {
        self.line = Some(line);
        self
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn line(&self) -> Option<NonZeroUsize> {
        self.line
    }
}

fn split_line_suffix(location: &str) -> Option<(&str, Option<NonZeroUsize>)> {
    let Some((path, last)) = location.rsplit_once(':') else {
        return Some((location, None));
    };
    if last.is_empty() || !last.bytes().all(|byte| byte.is_ascii_digit()) {
        return Some((location, None));
    }
    if let Some((path, line)) = path.rsplit_once(':')
        && !line.is_empty()
        && line.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Some((path, Some(line.parse().ok()?)));
    }
    Some((path, Some(last.parse().ok()?)))
}

#[derive(Debug, Error)]
enum ReadFileError {
    #[error("无法读取文件：{0}")]
    Io(#[source] io::Error),
    #[error("仅支持普通文件，不能查看目录、FIFO、设备或套接字")]
    NotRegular,
    #[error("文件超过 2 MiB 的查看限制")]
    TooLarge,
    #[error("文件包含二进制控制字符，不能作为文本查看")]
    Binary,
    #[error("文件不是有效的 UTF-8 文本：{0}")]
    InvalidUtf8(#[source] Utf8Error),
}

impl ReadFileError {
    fn io(error: io::Error) -> Self {
        Self::Io(error)
    }

    fn not_regular() -> Self {
        Self::NotRegular
    }

    fn too_large() -> Self {
        Self::TooLarge
    }

    fn binary() -> Self {
        Self::Binary
    }

    fn invalid_utf8(error: Utf8Error) -> Self {
        Self::InvalidUtf8(error)
    }
}

#[derive(Debug)]
enum FileLoadState {
    Idle,
    Loading,
    Ready,
    Failed(ReadFileError),
}

impl FileLoadState {
    fn new() -> Self {
        Self::Idle
    }

    fn loading() -> Self {
        Self::Loading
    }

    fn ready() -> Self {
        Self::Ready
    }

    fn failed(error: ReadFileError) -> Self {
        Self::Failed(error)
    }

    fn is_ready(&self) -> bool {
        matches!(self, Self::Ready)
    }
}

fn open_regular_file(path: &Path) -> Result<fs::File, ReadFileError> {
    let metadata = fs::metadata(path).map_err(ReadFileError::io)?;
    if !metadata.is_file() {
        return Err(ReadFileError::not_regular());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.custom_flags(libc::O_NONBLOCK | libc::O_NOCTTY);
    }
    let file = options.open(path).map_err(ReadFileError::io)?;
    if !file.metadata().map_err(ReadFileError::io)?.is_file() {
        return Err(ReadFileError::not_regular());
    }
    Ok(file)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LineEnding {
    Lf,
    CrLf,
    Cr,
}

impl LineEnding {
    fn lf() -> Self {
        Self::Lf
    }

    fn crlf() -> Self {
        Self::CrLf
    }

    fn cr() -> Self {
        Self::Cr
    }

    fn text(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
            Self::Cr => "\r",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
            Self::Cr => "CR",
        }
    }
}

#[derive(Clone, Debug)]
struct FileDocument {
    metadata: fs::Metadata,
    original: Arc<[u8]>,
    content: SharedString,
    bom: bool,
    endings: Vec<LineEnding>,
    line_ending: LineEnding,
}

impl FileDocument {
    fn new(
        _source_path: PathBuf,
        location: PathBuf,
        original: Vec<u8>,
    ) -> Result<Self, ReadFileError> {
        let metadata = fs::metadata(&location).map_err(ReadFileError::io)?;
        let bom = original.starts_with(UTF8_BOM);
        Self::new_with_bom(metadata, original, bom)
    }

    fn new_with_bom(
        metadata: fs::Metadata,
        original: Vec<u8>,
        bom: bool,
    ) -> Result<Self, ReadFileError> {
        let bytes =
            if bom { original.strip_prefix(UTF8_BOM).unwrap_or(&original) } else { &original };
        if bytes.iter().any(|byte| matches!(byte, 0..=8 | 11 | 14..=31 | 127)) {
            return Err(ReadFileError::binary());
        }
        let text = std::str::from_utf8(bytes).map_err(ReadFileError::invalid_utf8)?;
        let mut content = String::with_capacity(text.len());
        let mut endings = Vec::new();
        let mut chars = text.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\r' => {
                    let ending = if chars.peek() == Some(&'\n') {
                        chars.next();
                        LineEnding::crlf()
                    } else {
                        LineEnding::cr()
                    };
                    endings.push(ending);
                    content.push('\n');
                },
                '\n' => {
                    endings.push(LineEnding::lf());
                    content.push('\n');
                },
                _ => content.push(ch),
            }
        }
        let line_ending = endings.first().copied().unwrap_or_else(LineEnding::lf);
        Ok(Self {
            metadata,
            original: original.into(),
            content: content.into(),
            bom,
            endings,
            line_ending,
        })
    }

    fn encode(&self, content: &str) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(content.len() + self.endings.len() + UTF8_BOM.len());
        if self.bom {
            bytes.extend_from_slice(UTF8_BOM);
        }
        let content = content.replace("\r\n", "\n").replace('\r', "\n");
        let mut endings =
            vec![self.line_ending; content.bytes().filter(|byte| *byte == b'\n').count()];
        if self.endings.iter().any(|ending| *ending != self.line_ending) {
            let original = self.content.replace("\r\n", "\n").replace('\r', "\n");
            let diff = similar::TextDiff::configure()
                .timeout(Duration::from_millis(200))
                .diff_lines(&original, &content);
            for op in diff.ops() {
                for (old, new) in op.old_range().zip(op.new_range()) {
                    if let Some(ending) = self.endings.get(old)
                        && let Some(destination) = endings.get_mut(new)
                    {
                        *destination = *ending;
                    }
                }
            }
        }
        for (ix, line) in content.split_inclusive('\n').enumerate() {
            if let Some(line) = line.strip_suffix('\n') {
                bytes.extend_from_slice(line.as_bytes());
                bytes.extend_from_slice(endings[ix].text().as_bytes());
            } else {
                bytes.extend_from_slice(line.as_bytes());
            }
        }
        bytes
    }

    fn ending_label(&self) -> &'static str {
        if self.endings.iter().any(|ending| *ending != self.line_ending) {
            "混合换行"
        } else {
            self.line_ending.label()
        }
    }
}

fn read_text_file(path: &Path) -> Result<FileDocument, ReadFileError> {
    let location = fs::canonicalize(path).map_err(ReadFileError::io)?;
    let file = open_regular_file(&location)?;
    let metadata = file.metadata().map_err(ReadFileError::io)?;
    if metadata.len() > MAX_FILE_BYTES {
        return Err(ReadFileError::too_large());
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).map_err(ReadFileError::io)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ReadFileError::too_large());
    }
    let mut document = FileDocument::new(path.to_owned(), location, bytes)?;
    document.metadata = metadata;
    Ok(document)
}

#[derive(Debug, Error)]
enum SaveFileError {
    #[error("保存失败，草稿仍保留：{}：{source}", path.display())]
    Io { path: PathBuf, source: io::Error },
    #[error("文件已被外部修改或移动，未覆盖。草稿仍保留，请复制草稿并核对磁盘文件。")]
    Conflict,
    #[error("文件为只读，未保存。草稿仍保留，请检查文件权限。")]
    Readonly,
    #[error("无法检查磁盘文件，未保存。草稿仍保留：{0}")]
    Read(#[source] ReadFileError),
}

impl SaveFileError {
    fn io(path: &Path, source: io::Error) -> Self {
        Self::Io { path: path.to_owned(), source }
    }

    fn conflict() -> Self {
        Self::Conflict
    }

    fn readonly() -> Self {
        Self::Readonly
    }

    fn read(error: ReadFileError) -> Self {
        Self::Read(error)
    }
}

fn same_file_identity(first: &fs::Metadata, second: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        first.dev() == second.dev() && first.ino() == second.ino()
    }
    #[cfg(not(unix))]
    {
        first.created().is_ok_and(|created| second.created().is_ok_and(|other| created == other))
    }
}

fn same_file_version(first: &fs::Metadata, second: &fs::Metadata) -> bool {
    if !same_file_identity(first, second)
        || first.len() != second.len()
        || first.modified().ok() != second.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        first.ctime() == second.ctime() && first.ctime_nsec() == second.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn check_original(path: &Path, original: &[u8]) -> Result<fs::Metadata, SaveFileError> {
    let file = open_regular_file(path).map_err(SaveFileError::read)?;
    let before = file.metadata().map_err(|error| SaveFileError::io(path, error))?;
    if before.len() != original.len() as u64 {
        return Err(SaveFileError::conflict());
    }
    let mut bytes = Vec::with_capacity(original.len());
    (&file)
        .take(original.len() as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| SaveFileError::io(path, error))?;
    let after = file.metadata().map_err(|error| SaveFileError::io(path, error))?;
    let current = fs::symlink_metadata(path).map_err(|error| SaveFileError::io(path, error))?;
    if bytes != original
        || !current.is_file()
        || !same_file_version(&before, &after)
        || !same_file_version(&after, &current)
    {
        return Err(SaveFileError::conflict());
    }
    Ok(current)
}

fn create_temporary(location: &Path) -> Result<(PathBuf, fs::File), SaveFileError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    for _ in 0..16 {
        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let name = format!(".aries-save-{}-{sequence}.tmp", std::process::id());
        let temporary = location.with_file_name(name);
        match options.open(&temporary) {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(SaveFileError::io(&temporary, error)),
        }
    }
    Err(SaveFileError::io(
        location,
        io::Error::new(io::ErrorKind::AlreadyExists, "无法创建唯一的保存临时文件"),
    ))
}

fn save_document(
    path: &Path,
    document: &FileDocument,
    content: SharedString,
) -> Result<FileDocument, SaveFileError> {
    let location = fs::canonicalize(path).map_err(|error| SaveFileError::io(path, error))?;
    let metadata = check_original(&location, &document.original)?;
    if !same_file_identity(&document.metadata, &metadata) {
        return Err(SaveFileError::conflict());
    }
    if metadata.permissions().readonly() {
        return Err(SaveFileError::readonly());
    }
    let bytes = document.encode(&content);
    let mut saved = FileDocument::new_with_bom(metadata.clone(), bytes, document.bom)
        .map_err(SaveFileError::read)?;
    saved.content = content;
    let (temporary, mut file) = create_temporary(&location)?;
    let result = (|| {
        #[cfg(target_os = "macos")]
        fs::copy(&location, &temporary).map_err(|error| SaveFileError::io(&temporary, error))?;
        file.set_len(0).map_err(|error| SaveFileError::io(&temporary, error))?;
        file.write_all(&saved.original).map_err(|error| SaveFileError::io(&temporary, error))?;
        file.set_permissions(metadata.permissions())
            .map_err(|error| SaveFileError::io(&temporary, error))?;
        file.sync_all().map_err(|error| SaveFileError::io(&temporary, error))?;
        let saved_metadata =
            file.metadata().map_err(|error| SaveFileError::io(&temporary, error))?;
        drop(file);
        let current = check_original(&location, &document.original)?;
        if !same_file_version(&metadata, &current)
            || fs::canonicalize(path).map_err(|error| SaveFileError::io(path, error))? != location
        {
            return Err(SaveFileError::conflict());
        }
        fs::rename(&temporary, &location).map_err(|error| SaveFileError::io(&location, error))?;
        Ok(saved_metadata)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    saved.metadata = result?;
    Ok(saved)
}

#[derive(Debug, PartialEq, Eq)]
struct EditorStatus {
    line: u32,
    column: u32,
    selection: std::ops::Range<usize>,
    lines: usize,
}

impl EditorStatus {
    fn new(editor: &EditorState) -> Self {
        let position = editor.cursor_position();
        Self {
            line: position.line + 1,
            column: position.character + 1,
            selection: editor.selected_range(),
            lines: editor.text().lines_len(),
        }
    }
}

fn file_language(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" => "rust",
        "sh" | "bash" | "zsh" => "bash",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" | "hh" => "cpp",
        "css" => "css",
        "diff" | "patch" => "diff",
        "go" => "go",
        "html" | "htm" => "html",
        "java" => "java",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "json" => "json",
        "md" | "markdown" => "markdown",
        "py" | "pyi" => "python",
        "toml" => "toml",
        "tsx" => "tsx",
        "ts" | "mts" | "cts" => "typescript",
        "yaml" | "yml" => "yaml",
        _ => "plaintext",
    }
}

#[expect(
    clippy::field_reassign_with_default,
    reason = "TabSize only exposes its Default constructor"
)]
fn editor_tab_size() -> TabSize {
    let mut tab = TabSize::default();
    tab.tab_size = 4;
    tab
}

pub struct FileView {
    target: Option<OpenFile>,
    editor: Entity<EditorState>,
    state: FileLoadState,
    focus_handle: FocusHandle,
    revision: u64,
    load_task: Option<Task<()>>,
    feedback: Option<SharedString>,
    document: Option<FileDocument>,
    dirty: bool,
    saving: bool,
    save_error: Option<String>,
    status: EditorStatus,
    _subscriptions: Vec<Subscription>,
    _save_task: Option<Task<()>>,
    auto_save_task: Option<Task<()>>,
    auto_save_paused: bool,
}

impl FileView {
    pub fn init(cx: &mut App) {
        self::init(cx);
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let tab = editor_tab_size();
            EditorState::new(window, cx)
                .soft_wrap(false)
                .line_number(true)
                .tab_size(tab)
                .replaceable(true)
        });
        let status = EditorStatus::new(editor.read(cx));
        let change_subscription =
            cx.subscribe_in(&editor, window, |this, editor, event, window, cx| {
                if matches!(event, InputEvent::Blur) {
                    if !this.auto_save_paused && this.save_error.is_none() {
                        this.save(window, cx);
                    }
                    return;
                }
                if !matches!(event, InputEvent::Change) || !this.is_ready() {
                    return;
                }
                let content = editor.read(cx).value();
                let dirty =
                    this.document.as_ref().is_some_and(|document| document.content != content);
                let changed = this.dirty != dirty || this.feedback.is_some();
                this.dirty = dirty;
                this.feedback = None;
                this.schedule_auto_save(window, cx);
                if changed {
                    cx.notify();
                }
            });
        let status_subscription = cx.observe(&editor, |this, editor, cx| {
            let status = EditorStatus::new(editor.read(cx));
            if this.status != status {
                this.status = status;
                cx.notify();
            }
        });
        Self {
            target: None,
            editor,
            state: FileLoadState::new(),
            focus_handle: cx.focus_handle(),
            revision: 0,
            load_task: None,
            feedback: None,
            document: None,
            dirty: false,
            saving: false,
            save_error: None,
            status,
            _subscriptions: vec![change_subscription, status_subscription],
            _save_task: None,
            auto_save_task: None,
            auto_save_paused: false,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn is_saving(&self) -> bool {
        self.saving
    }

    pub fn save_error(&self) -> Option<&str> {
        self.save_error.as_deref()
    }

    fn schedule_auto_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.auto_save_task = None;
        if self.auto_save_paused || !self.is_ready() || !self.dirty {
            return;
        }
        let revision = self.revision;
        self.auto_save_task = Some(cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(AUTO_SAVE_DELAY).await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.revision == revision && !this.auto_save_paused {
                    this.save(window, cx);
                }
            });
        }));
    }

    pub fn pause_auto_save(&mut self) {
        self.auto_save_paused = true;
        self.auto_save_task = None;
    }

    pub fn resume_auto_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.auto_save_paused = false;
        if self.save_error.is_none() {
            self.schedule_auto_save(window, cx);
        }
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_saving() || !self.is_ready() || !self.is_dirty() {
            return;
        }
        let Some(document) = self.document.clone() else {
            return;
        };
        let Some(target) = self.target.as_ref() else {
            return;
        };
        self.auto_save_task = None;
        let path = target.path().to_owned();
        let content = self.editor.read(cx).value();
        let revision = self.revision;
        self.saving = true;
        self.save_error = None;
        self.feedback = None;
        let background =
            cx.background_spawn(async move { save_document(&path, &document, content) });
        self._save_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = background.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.revision != revision {
                    return;
                }
                this.saving = false;
                match result {
                    Ok(document) => {
                        this.dirty = document.content != this.editor.read(cx).value();
                        this.document = Some(document);
                        this.schedule_auto_save(window, cx);
                    },
                    Err(error) => this.save_error = Some(error.to_string()),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    pub fn retarget(&mut self, target: OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if !self.is_ready() {
            self.open(target, window, cx);
            return;
        }
        let language = file_language(target.path());
        self.target = Some(target);
        self.editor.update(cx, |editor, cx| editor.set_highlighter(language, cx));
        cx.notify();
    }

    pub fn is_open(&self) -> bool {
        self.target.is_some()
    }

    pub fn target(&self) -> Option<&OpenFile> {
        self.target.as_ref()
    }

    pub fn open(&mut self, target: OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        let revision = self.revision;
        let path = target.path().to_path_buf();
        self.load_task = None;
        self.auto_save_task = None;
        self.auto_save_paused = false;
        self.feedback = None;
        self.save_error = None;
        self.document = None;
        self.dirty = false;
        self.target = Some(target);
        self.state = FileLoadState::loading();
        self.editor.update(cx, |editor, cx| {
            editor.set_readonly(true, cx);
            editor.close_search(cx);
            editor.set_value("", window, cx);
            editor.set_search_query("", true, cx);
            editor.close_search(cx);
        });
        self.focus_handle.focus(window, cx);
        let task = cx.background_spawn(async move { read_text_file(&path) });
        self.load_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |this, cx| {
                    if this.revision != revision || !this.is_open() {
                        return;
                    }
                    match result {
                        Ok(document) => {
                            let Some(target) = this.target.as_ref() else {
                                return;
                            };
                            let language = file_language(target.path());
                            let line = target.line();
                            let focus_editor = this.focus_handle.is_focused(window);
                            this.editor.update(cx, |editor, cx| {
                                editor.set_highlighter(language, cx);
                                editor.set_value(document.content.clone(), window, cx);
                                editor.set_readonly(false, cx);
                                if let Some(line) = line {
                                    let row = (line.get() - 1)
                                        .min(editor.text().lines_len().saturating_sub(1));
                                    let offset = editor.text().line_start_offset(row);
                                    editor.set_selected_range(offset..offset, cx);
                                }
                                if focus_editor {
                                    editor.focus(window, cx);
                                }
                            });
                            this.status = EditorStatus::new(this.editor.read(cx));
                            this.document = Some(document);
                            this.state = FileLoadState::ready();
                        },
                        Err(error) => this.state = FileLoadState::failed(error),
                    }
                    cx.notify();
                })
            });
        }));
        cx.notify();
    }

    pub fn is_ready(&self) -> bool {
        self.state.is_ready()
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.state, FileLoadState::Loading)
    }

    pub fn has_error(&self) -> bool {
        matches!(self.state, FileLoadState::Failed(_))
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.is_ready() {
            self.editor.update(cx, |editor, cx| editor.focus(window, cx));
        } else {
            self.focus_handle.focus(window, cx);
        }
    }

    pub fn reveal(&mut self, target: OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        let Some(current) = self.target.as_mut() else {
            return;
        };
        if current.path() != target.path() {
            return;
        }
        current.line = target.line();
        if let Some(line) = current.line
            && self.state.is_ready()
        {
            self.editor.update(cx, |editor, cx| {
                let row = (line.get() - 1).min(editor.text().lines_len().saturating_sub(1));
                let offset = editor.text().line_start_offset(row);
                editor.set_selected_range(offset..offset, cx);
            });
        }
        self.focus(window, cx);
        cx.notify();
    }

    pub fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_dirty() || self.is_saving() {
            return;
        }
        self.reload_discarding(window, cx);
    }

    pub fn reload_discarding(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.saving {
            return;
        }
        if let Some(target) = self.target.clone() {
            self.open(target, window, cx);
        }
    }

    pub fn search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open() || !self.state.is_ready() {
            return;
        }
        self.editor.update(cx, |editor, cx| {
            editor.focus(window, cx);
            editor.open_search(false, cx);
        });
    }

    pub fn copy_all(&mut self, cx: &mut Context<Self>) {
        if !self.is_open() || !self.state.is_ready() {
            return;
        }
        let text = self.editor.read(cx).text().to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.feedback = Some("已复制全文".into());
        cx.notify();
    }

    fn navigate_match(&mut self, previous: bool, cx: &mut Context<Self>) {
        if !self.state.is_ready() || !self.editor.read(cx).search_session().is_active() {
            return;
        }
        self.editor.update(cx, |editor, cx| {
            if previous {
                let _ = editor.previous_search_match(cx);
            } else {
                let _ = editor.next_search_match(cx);
            }
        });
    }

    fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.read(cx).search_session().open {
            self.editor.update(cx, |editor, cx| {
                editor.close_search(cx);
                editor.focus(window, cx);
            });
        }
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let path: SharedString = self
            .target
            .as_ref()
            .map(|target| target.path().display().to_string())
            .unwrap_or_default()
            .into();
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap_2()
            .px_3()
            .py_1()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .id("file-view-path")
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(path.clone())
                    .tooltip(move |window, cx| {
                        gpui_kit::component::tooltip::Tooltip::new(path.clone()).build(window, cx)
                    }),
            )
    }

    fn render_content(&self, cx: &mut Context<Self>) -> AnyElement {
        match &self.state {
            FileLoadState::Ready => Editor::new(&self.editor)
                .appearance(false)
                .bordered(false)
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(cx.theme().mono_font_size)
                .aria_label("文件内容，编辑后自动保存")
                .h_full()
                .w_full()
                .into_any_element(),
            FileLoadState::Loading => div()
                .flex()
                .items_center()
                .gap_2()
                .p_3()
                .text_color(cx.theme().muted_foreground)
                .child(Spinner::new().small())
                .child("正在读取文件…")
                .into_any_element(),
            FileLoadState::Failed(error) => div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .child(div().text_color(cx.theme().danger).child(error.to_string()))
                .child(
                    Button::new("file-view-retry")
                        .small()
                        .label("重新读取")
                        .on_click(cx.listener(|this, _, window, cx| this.reload(window, cx))),
                )
                .into_any_element(),
            FileLoadState::Idle => div()
                .p_3()
                .text_color(cx.theme().muted_foreground)
                .child("从文件树选择文件以编辑内容")
                .into_any_element(),
        }
    }
}

impl EventEmitter<CloseFile> for FileView {}

impl Focusable for FileView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FileView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.is_open() {
            return Empty.into_any_element();
        }
        div()
            .id("file-view")
            .key_context(FILE_VIEW_CONTEXT)
            .track_focus(&self.focus_handle)
            .capture_action(cx.listener(|this, _: &Search, window, cx| {
                this.search(window, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                this.dismiss(window, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &FindFile, window, cx| this.search(window, cx)))
            .on_action(cx.listener(|_, _: &CloseFile, _, cx| cx.emit(CloseFile::new())))
            .on_action(cx.listener(|this, _: &CopyFile, _, cx| this.copy_all(cx)))
            .on_action(cx.listener(|this, _: &SaveFile, window, cx| this.save(window, cx)))
            .on_action(cx.listener(|this, _: &NextMatch, _, cx| this.navigate_match(false, cx)))
            .on_action(cx.listener(|this, _: &PreviousMatch, _, cx| this.navigate_match(true, cx)))
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_toolbar(cx))
            .child(div().flex_1().min_w_0().min_h_0().child(self.render_content(cx)))
            .when_some(self.save_error().map(str::to_owned), |view, error| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .px_3()
                        .py_1()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .flex_wrap()
                    .gap_3()
                    .px_3()
                    .py_1()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .when(self.state.is_ready(), |view| {
                        view.child(format!("行 {}，列 {}", self.status.line, self.status.column))
                            .when(!self.status.selection.is_empty(), |view| {
                                view.child(format!("已选择 {} 字节", self.status.selection.len()))
                            })
                            .child(format!("{} 行", self.status.lines))
                            .child("空格：4")
                            .child(file_language(
                                self.target.as_ref().map_or(Path::new(""), OpenFile::path),
                            ))
                    })
                    .when_some(self.document.as_ref(), |view, document| {
                        view.child(if document.bom { "UTF-8 BOM" } else { "UTF-8" })
                            .child(document.ending_label())
                    })
                    .when_some(self.feedback.clone(), |this, feedback| this.child(feedback)),
            )
            .into_any_element()
    }
}
