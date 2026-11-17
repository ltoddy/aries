use std::fs::{self, OpenOptions};
use std::io::{self, Read as _};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::str::Utf8Error;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Editor, EditorState, Escape, RopeExt, Search};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Disableable, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use thiserror::Error;
use url::Url;

const FILE_VIEW_CONTEXT: &str = "FileView";
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

actions!(file_view, [FindFile, CloseFile, CopyFile, NextMatch, PreviousMatch]);

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
    let (find, close, copy, next, previous) = if cfg!(target_os = "macos") {
        ("cmd-f", "cmd-w", "cmd-shift-c", "cmd-g", "cmd-shift-g")
    } else {
        ("ctrl-f", "ctrl-w", "ctrl-shift-c", "ctrl-g", "ctrl-shift-g")
    };
    cx.bind_keys([
        KeyBinding::new(find, FindFile::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(close, CloseFile::new(), Some(FILE_VIEW_CONTEXT)),
        KeyBinding::new(copy, CopyFile::new(), Some(FILE_VIEW_CONTEXT)),
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

fn read_text_file(path: &Path) -> Result<String, ReadFileError> {
    let file = open_regular_file(path)?;
    if file.metadata().map_err(ReadFileError::io)?.len() > MAX_FILE_BYTES {
        return Err(ReadFileError::too_large());
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).map_err(ReadFileError::io)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ReadFileError::too_large());
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    if bytes.iter().any(|byte| matches!(byte, 0..=8 | 11 | 14..=31 | 127)) {
        return Err(ReadFileError::binary());
    }
    let text = std::str::from_utf8(bytes).map_err(ReadFileError::invalid_utf8)?;
    Ok(text.to_owned())
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

pub struct FileView {
    target: Option<OpenFile>,
    editor: Entity<EditorState>,
    state: FileLoadState,
    focus_handle: FocusHandle,
    return_focus: Option<FocusHandle>,
    revision: u64,
    load_task: Option<Task<()>>,
    feedback: Option<SharedString>,
}

impl FileView {
    pub fn init(cx: &mut App) {
        self::init(cx);
    }

    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let mut editor = EditorState::new(window, cx).soft_wrap(false).replaceable(false);
            editor.set_readonly(true, cx);
            editor
        });
        Self {
            target: None,
            editor,
            state: FileLoadState::new(),
            focus_handle: cx.focus_handle(),
            return_focus: None,
            revision: 0,
            load_task: None,
            feedback: None,
        }
    }

    pub fn is_open(&self) -> bool {
        self.target.is_some()
    }

    pub fn target(&self) -> Option<&OpenFile> {
        self.target.as_ref()
    }

    pub fn open(&mut self, target: OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open() {
            self.return_focus = window.focused(cx);
        }
        self.revision = self.revision.wrapping_add(1);
        let revision = self.revision;
        let path = target.path().to_path_buf();
        self.load_task = None;
        self.feedback = None;
        self.target = Some(target);
        self.state = FileLoadState::loading();
        self.editor.update(cx, |editor, cx| {
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
                        Ok(text) => {
                            let Some(target) = this.target.as_ref() else {
                                return;
                            };
                            let language = file_language(target.path());
                            let line = target.line();
                            let focus_editor = this.focus_handle.is_focused(window);
                            this.editor.update(cx, |editor, cx| {
                                editor.set_highlighter(language, cx);
                                editor.set_value(text, window, cx);
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

    fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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

    pub fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_open() {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        self.target = None;
        self.load_task = None;
        self.state = FileLoadState::new();
        self.feedback = None;
        self.editor.update(cx, |editor, cx| {
            editor.close_search(cx);
            editor.set_value("", window, cx);
        });
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        } else {
            window.blur(cx);
        }
        cx.notify();
    }

    fn copy_all(&mut self, cx: &mut Context<Self>) {
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
        } else {
            self.close(window, cx);
        }
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .flex_shrink_0()
            .flex()
            .items_center()
            .flex_wrap()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                Button::new("file-view-search")
                    .small()
                    .ghost()
                    .label("搜索")
                    .disabled(!self.state.is_ready())
                    .tooltip_with_action("在文件内搜索", &FindFile::new(), Some(FILE_VIEW_CONTEXT))
                    .on_click(cx.listener(|this, _, window, cx| this.search(window, cx))),
            )
            .child(
                Button::new("file-view-copy-all")
                    .small()
                    .ghost()
                    .label("复制全文")
                    .disabled(!self.state.is_ready())
                    .tooltip_with_action("复制全文", &CopyFile::new(), Some(FILE_VIEW_CONTEXT))
                    .on_click(cx.listener(|this, _, _, cx| this.copy_all(cx))),
            )
            .child(
                Button::new("file-view-refresh")
                    .small()
                    .ghost()
                    .label("刷新文件")
                    .loading(matches!(self.state, FileLoadState::Loading))
                    .disabled(!self.is_open() || matches!(self.state, FileLoadState::Loading))
                    .on_click(cx.listener(|this, _, window, cx| this.reload(window, cx))),
            )
            .child(
                Button::new("file-view-close")
                    .small()
                    .ghost()
                    .label("关闭")
                    .tooltip_with_action(
                        "关闭文件查看器",
                        &CloseFile::new(),
                        Some(FILE_VIEW_CONTEXT),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
            )
            .into_any_element()
    }

    fn render_content(&self, cx: &mut Context<Self>) -> AnyElement {
        match &self.state {
            FileLoadState::Ready => Editor::new(&self.editor)
                .readonly(true)
                .appearance(false)
                .bordered(false)
                .aria_label("文件内容，只读，可选择并复制")
                .h_full()
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
                .child("从文件树选择文件以查看内容")
                .into_any_element(),
        }
    }
}

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
        let title = self
            .target
            .as_ref()
            .map(|target| target.path().display().to_string())
            .unwrap_or_default();
        div()
            .id("file-view")
            .key_context(FILE_VIEW_CONTEXT)
            .track_focus(&self.focus_handle)
            .capture_action(cx.listener(|this, _: &crate::CloseTab, window, cx| {
                this.close(window, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &Search, window, cx| {
                this.search(window, cx);
                cx.stop_propagation();
            }))
            .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                this.dismiss(window, cx);
                cx.stop_propagation();
            }))
            .on_action(cx.listener(|this, _: &FindFile, window, cx| this.search(window, cx)))
            .on_action(cx.listener(|this, _: &CloseFile, window, cx| this.close(window, cx)))
            .on_action(cx.listener(|this, _: &CopyFile, _, cx| this.copy_all(cx)))
            .on_action(cx.listener(|this, _: &NextMatch, _, cx| this.navigate_match(false, cx)))
            .on_action(cx.listener(|this, _: &PreviousMatch, _, cx| this.navigate_match(true, cx)))
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(div().flex_shrink_0().min_w_0().px_3().py_2().text_sm().truncate().child(title))
            .child(self.render_toolbar(cx))
            .child(div().flex_1().min_w_0().min_h_0().child(self.render_content(cx)))
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .flex_wrap()
                    .gap_3()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("只读")
                    .when(self.state.is_ready(), |this| {
                        this.child(format!(
                            "UTF-8 · {} 行",
                            self.editor.read(cx).text().lines_len()
                        ))
                    })
                    .when_some(self.feedback.clone(), |this, feedback| this.child(feedback)),
            )
            .into_any_element()
    }
}
