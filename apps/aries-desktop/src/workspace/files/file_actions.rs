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

use gpui_kit::base::actions::Confirm;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme, Disableable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;
#[cfg(any(target_os = "macos", target_os = "linux"))]
use rustix::fs::{self as fd_fs, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat};
use thiserror::Error;

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

#[derive(Clone)]
struct PreparedAction {
    command: FileCommand,
    root: PathBuf,
    root_metadata: Metadata,
    parent: PathBuf,
    parent_metadata: Metadata,
    entry_metadata: Option<Metadata>,
}

impl PreparedAction {
    fn new(command: FileCommand) -> Result<Self, FileActionError> {
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
    fn execute(&self, _: Option<&str>) -> Result<FileActionEvent, FileActionError> {
        self.revalidate()?;
        Err(FileActionError::unsupported("此平台没有可用的安全目录句柄操作接口，原条目保持不变。"))
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn execute(&self, name: Option<&str>) -> Result<FileActionEvent, FileActionError> {
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
                    form.command = prepared.command.clone();
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

    fn render_dialog(
        &self,
        dialog: Dialog,
        owner: WeakEntity<Self>,
        revision: u64,
        cx: &App,
    ) -> Dialog {
        let Some(form) = self.form.as_ref().filter(|form| form.revision == revision) else {
            return dialog.close_button(false).on_ok(|_, _, _| false);
        };
        let busy = self.is_busy();
        let completed = form.completed;
        let valid = form
            .input
            .as_ref()
            .is_none_or(|input| validate_basename(&input.read(cx).value()).is_ok());
        let confirm = owner.clone();
        let cancel = owner.clone();
        let close = owner.clone();
        let button_confirm = owner.clone();
        let action_cancel = owner.clone();
        let button_cancel = owner;
        let content = div().flex().flex_col().gap_2().min_w_0()
            .child(div().text_sm().text_color(cx.theme().muted_foreground).whitespace_normal()
                .child(form.command.path().display().to_string()))
            .when(form.command.is_delete(), |content| {
                let consequence = if form.prepared.as_ref().and_then(|prepared| prepared.entry_metadata.as_ref())
                    .is_some_and(|metadata| metadata.file_type().is_symlink()) {
                    "只删除此符号链接，不删除它指向的内容。不会移入废纸篓，无法撤销。"
                } else {
                    "永久删除此条目；如果是文件夹，将递归删除其中所有文件和子文件夹。不会移入废纸篓，无法撤销。"
                };
                content.child(consequence)
            })
            .when_some(form.input.as_ref(), |content, input| {
                content.child("名称").child(Input::new(input).aria_label("名称").disabled(busy || completed))
            })
            .when(busy, |content| content.child(if self.submitted { "正在操作，请勿重复提交…" } else { "正在校验路径…" }))
            .when(completed, |content| content.child("操作已完成"))
            .when_some(self.error.clone(), |content, error| {
                content.child(div().text_sm().text_color(cx.theme().danger).child(error))
            });
        dialog
            .title(form.command.title())
            .w(cx.theme().font_size * 28.)
            .overlay_closable(false)
            .close_button(!self.submitted)
            .on_ok(move |_, window, cx| {
                confirm.update(cx, |this, cx| this.confirm(revision, window, cx)).unwrap_or(false)
            })
            .on_cancel(move |_, _, cx| cancel_form(&cancel, revision, cx))
            .on_close(move |_, _, cx| {
                cancel_form(&close, revision, cx);
            })
            .child(content)
            .footer(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new(("cancel-file-action", revision))
                            .label("取消")
                            .disabled(self.submitted)
                            .on_action(move |_: &Confirm, window, cx| {
                                dismiss_form(&action_cancel, revision, window, cx);
                            })
                            .on_click(move |_, window, cx| {
                                dismiss_form(&button_cancel, revision, window, cx);
                            }),
                    )
                    .child(
                        Button::new(("confirm-file-action", revision))
                            .when(form.command.is_delete() && !completed, |button| button.danger())
                            .when(!form.command.is_delete() || completed, |button| button.primary())
                            .label(if completed { "完成" } else { form.command.label() })
                            .disabled(busy || (!completed && (form.prepared.is_none() || !valid)))
                            .on_click(move |_, window, cx| {
                                if button_confirm
                                    .update(cx, |this, cx| this.confirm(revision, window, cx))
                                    .unwrap_or(false)
                                {
                                    window.close_dialog(cx);
                                }
                            }),
                    ),
            )
    }
}

fn dismiss_form(owner: &WeakEntity<FileActions>, revision: u64, window: &mut Window, cx: &mut App) {
    cx.stop_propagation();
    if cancel_form(owner, revision, cx) {
        window.close_dialog(cx);
    }
}

fn cancel_form(owner: &WeakEntity<FileActions>, revision: u64, cx: &mut App) -> bool {
    owner
        .update(cx, |this, cx| {
            if this.submitted || this.form.as_ref().is_none_or(|form| form.revision != revision) {
                return false;
            }
            this.clear_form(cx);
            true
        })
        .unwrap_or(true)
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

fn require_absolute(path: &Path) -> Result<(), FileActionError> {
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

fn reject_git(path: &Path) -> Result<(), FileActionError> {
    if path.components().any(|component| {
        matches!(component, Component::Normal(name) if name.to_string_lossy().eq_ignore_ascii_case(".git"))
    }) {
        return Err(FileActionError::refused("不能修改 .git 或其内部条目。"));
    }
    Ok(())
}

fn check_containment(root: &Path, parent: &Path) -> Result<(), FileActionError> {
    if !parent.starts_with(root) {
        return Err(FileActionError::refused("父目录不在授权范围内。"));
    }
    reject_git(parent)
}

fn entry_metadata(path: &Path) -> Result<Metadata, FileActionError> {
    fs::symlink_metadata(path).map_err(|error| FileActionError::io(path, error))
}

fn canonical_directory(path: &Path) -> Result<PathBuf, FileActionError> {
    let canonical = fs::canonicalize(path).map_err(|error| FileActionError::io(path, error))?;
    if !entry_metadata(&canonical)?.is_dir() {
        return Err(FileActionError::refused(format!("{} 不是目录。", path.display())));
    }
    Ok(canonical)
}

fn check_identity(path: &Path, expected: &Metadata) -> Result<(), FileActionError> {
    if !same_entry(&entry_metadata(path)?, expected) {
        return Err(FileActionError::refused(format!(
            "{} 已改变，请取消后重新选择。",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn same_entry(left: &Metadata, right: &Metadata) -> bool {
    left.dev() == right.dev() && left.ino() == right.ino() && left.file_type() == right.file_type()
}

#[cfg(windows)]
fn same_entry(left: &Metadata, right: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    left.creation_time() == right.creation_time()
        && left.file_attributes() == right.file_attributes()
        && left.file_type() == right.file_type()
}

#[cfg(not(any(unix, windows)))]
fn same_entry(_: &Metadata, _: &Metadata) -> bool {
    false
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn check_file_identity(
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
fn open_directory_at(parent: &File, name: &OsStr, path: &Path) -> Result<File, FileActionError> {
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
fn rename_without_replacement(
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
fn delete_at(
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
