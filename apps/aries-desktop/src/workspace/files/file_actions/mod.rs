use std::io;
use std::path::{Path, PathBuf};

use gpui_kit::component::WindowExt;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::*;
use gpui_kit::*;
use thiserror::Error;

mod form;
mod fs;
mod prepare;

use self::prepare::PreparedAction;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileCommand {
    CreateFile { root: PathBuf, parent: PathBuf },
    CreateDirectory { root: PathBuf, parent: PathBuf },
    Rename { root: PathBuf, path: PathBuf },
    Delete { root: PathBuf, path: PathBuf },
}

impl FileCommand {
    pub fn create_file(root: impl Into<PathBuf>, parent: impl Into<PathBuf>) -> Self {
        Self::CreateFile { root: root.into(), parent: parent.into() }
    }

    pub fn create_directory(root: impl Into<PathBuf>, parent: impl Into<PathBuf>) -> Self {
        Self::CreateDirectory { root: root.into(), parent: parent.into() }
    }

    pub fn rename(root: impl Into<PathBuf>, path: impl Into<PathBuf>) -> Self {
        Self::Rename { root: root.into(), path: path.into() }
    }

    pub fn delete(root: impl Into<PathBuf>, path: impl Into<PathBuf>) -> Self {
        Self::Delete { root: root.into(), path: path.into() }
    }

    fn title(&self) -> String {
        match self {
            Self::CreateFile { .. } => "新建文件".into(),
            Self::CreateDirectory { .. } => "新建文件夹".into(),
            Self::Rename { path, .. } => format!("重命名“{}”", display_name(path)),
            Self::Delete { path, .. } => format!("永久删除“{}”？", display_name(path)),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Rename { .. } => "重命名",
            Self::Delete { .. } => "永久删除",
            Self::CreateFile { .. } | Self::CreateDirectory { .. } => "创建",
        }
    }

    fn path(&self) -> &Path {
        match self {
            Self::Rename { path, .. } | Self::Delete { path, .. } => path,
            Self::CreateFile { parent, .. } | Self::CreateDirectory { parent, .. } => parent,
        }
    }

    fn is_delete(&self) -> bool {
        matches!(self, Self::Delete { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileActionEvent {
    Created(PathBuf),
    Renamed { from: PathBuf, to: PathBuf },
    Deleted(PathBuf),
    Refresh(PathBuf),
}

impl FileActionEvent {
    pub fn created(path: impl Into<PathBuf>) -> Self {
        Self::Created(path.into())
    }

    pub fn renamed(from: impl Into<PathBuf>, to: impl Into<PathBuf>) -> Self {
        Self::Renamed { from: from.into(), to: to.into() }
    }

    pub fn deleted(path: impl Into<PathBuf>) -> Self {
        Self::Deleted(path.into())
    }

    pub fn refresh(path: impl Into<PathBuf>) -> Self {
        Self::Refresh(path.into())
    }
}

#[derive(Debug, Error)]
enum FileActionError {
    #[error("无法操作 {path}：{source}。请检查路径和权限后重试。")]
    Io { path: PathBuf, source: io::Error },
    #[error("名称无效：{0}")]
    InvalidName(&'static str),
    #[error("拒绝操作：{0}")]
    Refused(String),
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    #[error("此平台无法完成操作：{0}")]
    Unsupported(&'static str),
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    #[error("删除 {path} 未完成，部分内容可能已永久删除。请刷新并检查后再操作：{source}。")]
    PartialDelete { path: PathBuf, source: io::Error },
}

impl FileActionError {
    fn io(path: &Path, source: io::Error) -> Self {
        Self::Io { path: path.to_owned(), source }
    }

    fn invalid_name(reason: &'static str) -> Self {
        Self::InvalidName(reason)
    }

    fn refused(reason: impl Into<String>) -> Self {
        Self::Refused(reason.into())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn unsupported(reason: &'static str) -> Self {
        Self::Unsupported(reason)
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn partial_delete(path: &Path, source: io::Error) -> Self {
        Self::PartialDelete { path: path.to_owned(), source }
    }
}

struct FileActionForm {
    revision: u64,
    command: FileCommand,
    prepared: Option<PreparedAction>,
    input: Option<Entity<InputState>>,
    _subscription: Option<Subscription>,
    dialog_focus: Option<FocusHandle>,
    completed: bool,
}

impl FileActionForm {
    fn new(revision: u64, command: FileCommand, input: Option<Entity<InputState>>) -> Self {
        Self {
            revision,
            command,
            prepared: None,
            input,
            _subscription: None,
            dialog_focus: None,
            completed: false,
        }
    }
}

pub struct FileActions {
    form: Option<FileActionForm>,
    task: Option<Task<()>>,
    error: Option<String>,
    revision: u64,
    submitted: bool,
}

impl EventEmitter<FileActionEvent> for FileActions {}

impl FileActions {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self { form: None, task: None, error: None, revision: 0, submitted: false }
    }

    pub fn is_busy(&self) -> bool {
        self.task.is_some()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn execute(&mut self, command: FileCommand, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_busy() || self.form.is_some() || window.has_active_dialog(cx) {
            return;
        }
        self.revision = self.revision.wrapping_add(1);
        self.error = None;
        let revision = self.revision;
        let input = (!command.is_delete()).then(|| cx.new(|cx| InputState::new(window, cx)));
        if let (Some(input), FileCommand::Rename { path, .. }) = (&input, &command)
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            input.update(cx, |input, cx| input.set_value(name, window, cx));
        }
        let mut form = FileActionForm::new(revision, command.clone(), input.clone());
        if let Some(input) = &input {
            form._subscription =
                Some(cx.subscribe_in(input, window, move |_, _, event, window, cx| {
                    let enter = matches!(event, InputEvent::PressEnter { .. });
                    let change = matches!(event, InputEvent::Change);
                    if !enter && !change {
                        return;
                    }
                    let owner = cx.weak_entity();
                    window.defer(cx, move |window, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            if this.form.as_ref().is_none_or(|form| form.revision != revision) {
                                return;
                            }
                            if enter {
                                if this.confirm(revision, window, cx) {
                                    window.close_dialog(cx);
                                }
                            } else if !this.is_busy() {
                                this.error = this
                                    .form
                                    .as_ref()
                                    .and_then(|form| form.input.as_ref())
                                    .and_then(|input| {
                                        validate_basename(&input.read(cx).value()).err()
                                    })
                                    .map(|error| error.to_string());
                                window.refresh();
                                cx.notify();
                            }
                        });
                    });
                }));
        }
        self.form = Some(form);
        let owner = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(entity) = owner.upgrade() else {
                return dialog.on_ok(|_, _, _| false);
            };
            entity.read(cx).render_dialog(dialog, owner.clone(), revision, cx)
        });
        if let Some(form) = &mut self.form {
            form.dialog_focus = window.focused(cx);
        }
        if let Some(input) = input {
            let owner = cx.weak_entity();
            window.on_next_frame(move |window, cx| {
                if owner.upgrade().is_some_and(|owner| {
                    owner.read(cx).form.as_ref().is_some_and(|form| {
                        form.revision == revision
                            && form.dialog_focus.as_ref().is_some_and(|focus| {
                                focus.is_focused(window) || focus.contains_focused(window, cx)
                            })
                    })
                }) {
                    input.update(cx, |input, cx| input.focus(window, cx));
                }
            });
        }
        let task = cx.background_spawn(async move { PreparedAction::new(command) });
        self.task = Some(cx.spawn_in(window, async move |owner, cx| {
            let result = task.await;
            let mut result = Some(result);
            let delivered = owner.update_in(cx, |this, window, cx| {
                if let Some(result) = result.take() {
                    this.prepared(revision, result, cx);
                    window.refresh();
                }
            });
            if delivered.is_err() {
                let _ = owner.update(cx, |this, cx| {
                    if let Some(result) = result.take() {
                        this.prepared(revision, result, cx);
                        this.form = None;
                    }
                });
            }
        }));
        cx.notify();
    }

    pub fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.submitted {
            return false;
        }
        if self.form.is_none() {
            return !self.is_busy();
        }
        let owns_focus =
            self.form.as_ref().and_then(|form| form.dialog_focus.as_ref()).is_some_and(|focus| {
                focus.is_focused(window) || focus.contains_focused(window, cx)
            });
        if !owns_focus && window.has_active_dialog(cx) {
            return false;
        }
        self.clear_form(cx);
        if owns_focus {
            window.close_dialog(cx);
        }
        true
    }

    fn clear_form(&mut self, cx: &mut Context<Self>) {
        self.revision = self.revision.wrapping_add(1);
        self.task = None;
        self.form = None;
        self.error = None;
        self.submitted = false;
        cx.notify();
    }

    fn prepared(
        &mut self,
        revision: u64,
        result: Result<PreparedAction, FileActionError>,
        cx: &mut Context<Self>,
    ) {
        if self.revision != revision {
            return;
        }
        self.task = None;
        if let Some(form) = &mut self.form {
            match result {
                Ok(prepared) => {
                    form.command = prepared.command().clone();
                    form.prepared = Some(prepared);
                },
                Err(error) => self.error = Some(error.to_string()),
            }
        }
        cx.notify();
    }

    fn confirm(&mut self, revision: u64, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.is_busy() {
            return false;
        }
        let Some(form) = self.form.as_ref().filter(|form| form.revision == revision) else {
            return false;
        };
        if form.completed {
            self.clear_form(cx);
            return true;
        }
        let Some(prepared) = form.prepared.clone() else {
            return false;
        };
        let name = form.input.as_ref().map(|input| input.read(cx).value().to_string());
        if let Some(name) = &name
            && let Err(error) = validate_basename(name)
        {
            self.error = Some(error.to_string());
            window.refresh();
            cx.notify();
            return false;
        }
        self.error = None;
        self.submitted = true;
        let task = cx.background_spawn(async move { prepared.execute(name.as_deref()) });
        self.task = Some(cx.spawn_in(window, async move |owner, cx| {
            let result = task.await;
            let mut result = Some(result);
            let delivered = owner.update_in(cx, |this, window, cx| {
                if let Some(result) = result.take() {
                    let owns_focus =
                        this.form.as_ref().and_then(|form| form.dialog_focus.as_ref()).is_some_and(
                            |focus| focus.is_focused(window) || focus.contains_focused(window, cx),
                        );
                    let success = this.finished(revision, result, cx);
                    if success && owns_focus {
                        this.form = None;
                        window.close_dialog(cx);
                    }
                    window.refresh();
                }
            });
            if delivered.is_err() {
                let _ = owner.update(cx, |this, cx| {
                    if let Some(result) = result.take() {
                        this.finished(revision, result, cx);
                        this.form = None;
                    }
                });
            }
        }));
        window.refresh();
        cx.notify();
        false
    }

    fn finished(
        &mut self,
        revision: u64,
        result: Result<FileActionEvent, FileActionError>,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.revision != revision {
            return false;
        }
        self.task = None;
        self.submitted = false;
        let success = match result {
            Ok(event) => {
                if let Some(form) = &mut self.form {
                    form.completed = true;
                }
                cx.emit(event);
                true
            },
            Err(error) => {
                self.error = Some(error.to_string());
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                if let FileActionError::PartialDelete { path, .. } = &error {
                    cx.emit(FileActionEvent::refresh(path.clone()));
                }
                false
            },
        };
        cx.notify();
        success
    }
}

fn display_name(path: &Path) -> String {
    path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()
}

fn validate_basename(name: &str) -> Result<(), FileActionError> {
    if name.trim().is_empty() || matches!(name, "." | "..") {
        return Err(FileActionError::invalid_name("不能为空、. 或 ..。"));
    }
    if name.contains(['/', '\\', '\0']) {
        return Err(FileActionError::invalid_name("不能包含路径分隔符或空字符。"));
    }
    if name.eq_ignore_ascii_case(".git") {
        return Err(FileActionError::invalid_name("不能创建或替换 .git。"));
    }
    #[cfg(target_os = "macos")]
    if name.contains(':') {
        return Err(FileActionError::invalid_name("不能包含冒号。"));
    }
    #[cfg(windows)]
    {
        if name.contains(['<', '>', ':', '"', '|', '?', '*'])
            || name.chars().any(|ch| ch < ' ')
            || name.ends_with(['.', ' '])
        {
            return Err(FileActionError::invalid_name(
                "包含 Windows 不允许的字符，或以点、空格结尾。",
            ));
        }
        let stem = name.split('.').next().unwrap_or_default().trim_end().to_uppercase();
        let device = stem.strip_prefix("COM").or_else(|| stem.strip_prefix("LPT"));
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$")
            || device.is_some_and(|number| {
                matches!(
                    number,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            })
        {
            return Err(FileActionError::invalid_name("不能使用 Windows 保留的设备名称。"));
        }
    }
    Ok(())
}
