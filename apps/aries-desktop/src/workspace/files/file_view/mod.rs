use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use gpui_kit::component::ActiveTheme;
use gpui_kit::component::input::{EditorState, Escape, Search};
use gpui_kit::prelude::*;
use gpui_kit::*;
use url::Url;

mod document;
mod refresh;
mod status;
mod view;
mod watch;

use self::document::{FileDocument, ImageDocument, ReadFileError};
use self::status::{EditorStatus, file_language};

const FILE_VIEW_CONTEXT: &str = "FileView";
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const AUTO_SAVE_DELAY: Duration = Duration::from_millis(500);
const UTF8_BOM: &[u8] = &[0xef, 0xbb, 0xbf];

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

#[derive(Debug)]
enum FileLoadState {
    Idle,
    Loading,
    Text,
    Image,
    Failed(ReadFileError),
}

impl FileLoadState {
    fn new() -> Self {
        Self::Idle
    }

    fn loading() -> Self {
        Self::Loading
    }

    fn text() -> Self {
        Self::Text
    }

    fn image() -> Self {
        Self::Image
    }

    fn failed(error: ReadFileError) -> Self {
        Self::Failed(error)
    }

    fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    fn is_image(&self) -> bool {
        matches!(self, Self::Image)
    }
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
    image: Option<ImageDocument>,
    dirty: bool,
    saving: bool,
    save_error: Option<String>,
    status: EditorStatus,
    _subscriptions: Vec<Subscription>,
    _save_task: Option<Task<()>>,
    auto_save_task: Option<Task<()>>,
    auto_save_paused: bool,
    watch_task: Option<Task<()>>,
    watch_error: Option<String>,
    watch_identity: Option<Option<PathBuf>>,
    refresh_task: Option<Task<()>>,
    refresh_error: Option<String>,
    refresh_pending: bool,
    refresh_revision: u64,
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
            .when_some(
                self.save_error()
                    .or(self.refresh_error.as_deref())
                    .or(self.watch_error.as_deref())
                    .map(str::to_owned),
                |view, error| {
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
                },
            )
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
                    .when(self.state.is_text(), |view| {
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
                    .when(self.state.is_image(), |view| {
                        view.when_some(self.image.as_ref(), |view, image| {
                            view.child(image.label)
                                .when_some(image.dimensions, |view, (width, height)| {
                                    view.child(format!("{width} × {height}"))
                                })
                                .child(format!("{:.1} KiB", image.bytes as f64 / 1024.))
                                .child("只读预览")
                        })
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
