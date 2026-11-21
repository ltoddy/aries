use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use aries_extension::skill::Frontmatter;
use aries_filesystem::document::{DocumentError, FrontmatterDocument};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Editor, EditorState, InputEvent};
use gpui_kit::component::resizable::{ResizableState, h_resizable, resizable_panel};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Disableable, Selectable, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use thiserror::Error;

use crate::workspace::settings::typography;

const AUTOSAVE_DELAY: Duration = Duration::from_millis(700);

#[derive(Clone, Default, PartialEq, Eq, Action)]
pub struct SaveSkill;

impl SaveSkill {
    pub fn new() -> Self {
        Self
    }
}

pub enum SkillEditorEvent {
    Saved,
    Closed,
}

impl SkillEditorEvent {
    fn saved() -> Self {
        Self::Saved
    }

    fn closed() -> Self {
        Self::Closed
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MarkdownModeKind {
    Editor,
    Split,
    Preview,
}

impl MarkdownModeKind {
    fn editor() -> Self {
        Self::Editor
    }

    fn split() -> Self {
        Self::Split
    }

    fn preview() -> Self {
        Self::Preview
    }
}

pub struct SkillEditor {
    location: PathBuf,
    original: Option<SharedString>,
    input: Entity<EditorState>,
    preview: SharedString,
    mode: MarkdownModeKind,
    layout: Entity<ResizableState>,
    focus_handle: FocusHandle,
    dirty: bool,
    pending: bool,
    closing: bool,
    shutting_down: bool,
    error: Option<String>,
    _subscription: Subscription,
    _quit_subscription: Subscription,
    _task: Option<Task<()>>,
    _autosave_task: Option<Task<()>>,
}

impl EventEmitter<SkillEditorEvent> for SkillEditor {}

impl SkillEditor {
    pub fn new(location: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| EditorState::new(window, cx).language("markdown").soft_wrap(false));
        let subscription = cx.subscribe(&input, |this, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                let content = input.read(cx).value();
                this.preview = preview(&content);
                this.dirty = this.original.as_ref().is_some_and(|original| *original != content);
                this.error = None;
                this.schedule_save(cx);
                cx.notify();
            }
        });
        let quit_subscription = cx.on_app_quit(|this, cx| {
            this._autosave_task = None;
            this.shutting_down = true;
            let task = this._task.take();
            let owner = cx.entity();
            let executor = cx.background_executor().clone();
            let mut app = cx.to_async();
            async move {
                if let Some(task) = task {
                    task.await;
                }
                let snapshot = owner.update(&mut app, |this, cx| {
                    let original = this.original.clone()?;
                    let content = this.input.read(cx).value();
                    (original != content).then(|| (this.location.clone(), original, content))
                });
                if let Some((location, original, content)) = snapshot {
                    let result = executor
                        .spawn(async move { save_skill(&location, &original, &content) })
                        .await;
                    if let Err(error) = result {
                        crate::error::log("退出时无法保存 Skill", &error);
                    }
                }
            }
        });
        let mut this = Self {
            location,
            original: None,
            input,
            preview: SharedString::default(),
            mode: MarkdownModeKind::split(),
            layout: cx.new(|_| ResizableState::default()),
            focus_handle: cx.focus_handle(),
            dirty: false,
            pending: false,
            closing: false,
            shutting_down: false,
            error: None,
            _subscription: subscription,
            _quit_subscription: quit_subscription,
            _task: None,
            _autosave_task: None,
        };
        this.load(window, cx);
        this
    }

    pub fn is_focused(&self, window: &Window, cx: &App) -> bool {
        self.focus_handle.contains_focused(window, cx)
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == MarkdownModeKind::Preview {
            self.focus_handle.focus(window, cx);
        } else {
            self.input.update(cx, |input, cx| input.focus(window, cx));
        }
    }

    fn load(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.pending {
            return;
        }
        self.pending = true;
        self.error = None;
        let location = self.location.clone();
        let background = cx.background_executor().spawn(async move {
            let location = fs::canonicalize(&location)
                .map_err(|error| EditSkillError::io(&location, error))?;
            let content = fs::read_to_string(&location)
                .map_err(|error| EditSkillError::io(&location, error))?;
            Ok::<_, EditSkillError>((location, content))
        });
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = background.await;
            let _ = this.update_in(cx, |this, window, cx| {
                this.pending = false;
                match result {
                    Ok((location, content)) => {
                        this.location = location;
                        this.preview = preview(&content);
                        this.original = Some(content.clone().into());
                        this.input.update(cx, |input, cx| {
                            input.set_value(content, window, cx);
                        });
                    },
                    Err(error) => this.error = Some(crate::error::details(&error)),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn schedule_save(&mut self, cx: &mut Context<Self>) {
        self._autosave_task = None;
        if !self.dirty || self.closing || self.shutting_down {
            return;
        }
        self._autosave_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DELAY).await;
            let _ = this.update(cx, |this, cx| this.save(cx));
        }));
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        self._autosave_task = None;
        if self.pending || !self.dirty {
            return;
        }
        let Some(original) = self.original.clone() else { return };
        let content = self.input.read(cx).value();
        self.pending = true;
        self.error = None;
        let location = self.location.clone();
        let background = cx.background_executor().spawn(async move {
            let result = save_skill(&location, &original, &content);
            (content, result)
        });
        self._task = Some(cx.spawn(async move |this, cx| {
            let (content, result) = background.await;
            let _ = this.update(cx, |this, cx| {
                this.pending = false;
                let changed = content != this.input.read(cx).value();
                match result {
                    Ok(()) => {
                        this.original = Some(content);
                        this.dirty = changed;
                        if this.shutting_down {
                            cx.notify();
                            return;
                        }
                        cx.emit(SkillEditorEvent::saved());
                        if this.closing && !this.dirty {
                            cx.emit(SkillEditorEvent::closed());
                        } else if this.closing {
                            this.save(cx);
                        } else if this.dirty {
                            this.schedule_save(cx);
                        }
                    },
                    Err(error) => {
                        crate::error::log("无法自动保存 Skill", &error);
                        this.error = Some(crate::error::details(&error));
                        if this.shutting_down {
                            cx.notify();
                            return;
                        }
                        if changed && this.closing && this.dirty {
                            this.save(cx);
                        } else if changed && this.closing {
                            cx.emit(SkillEditorEvent::closed());
                        } else {
                            this.closing = false;
                            if changed {
                                this.schedule_save(cx);
                            }
                        }
                    },
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn close(&mut self, cx: &mut Context<Self>) {
        if self.pending || self.dirty {
            self.closing = true;
            self.save(cx);
            cx.notify();
        } else {
            cx.emit(SkillEditorEvent::closed());
        }
    }
}

impl Render for SkillEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loaded = self.original.is_some();
        let path: SharedString = self.location.display().to_string().into();
        let source = div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .when(self.mode == MarkdownModeKind::Split, |view| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .px_3()
                        .py_1()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("源文件（frontmatter 与正文）"),
                )
            })
            .child(
                div().flex_1().min_h_0().min_w_0().child(
                    Editor::new(&self.input)
                        .appearance(false)
                        .bordered(false)
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(typography::code_size())
                        .h_full()
                        .w_full()
                        .disabled(self.closing || !loaded)
                        .aria_label("Skill 源文件"),
                ),
            );
        let preview = div()
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .when(self.mode == MarkdownModeKind::Split, |view| {
                view.child(
                    div()
                        .flex_shrink_0()
                        .px_3()
                        .py_1()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("Markdown 预览"),
                )
            })
            .child(
                div().flex_1().min_h_0().min_w_0().child(
                    TextView::markdown("skill-preview", self.preview.clone())
                        .size_full()
                        .p_3()
                        .map(typography::style_markdown)
                        .selectable(true)
                        .scrollable(true),
                ),
            );
        let content = match self.mode {
            MarkdownModeKind::Editor => source.into_any_element(),
            MarkdownModeKind::Split => h_resizable("skill-editor-layout")
                .with_state(&self.layout)
                .child(
                    resizable_panel()
                        .size_range(cx.theme().font_size * 8. ..Pixels::MAX)
                        .child(source),
                )
                .child(
                    resizable_panel()
                        .size_range(cx.theme().font_size * 8. ..Pixels::MAX)
                        .child(preview),
                )
                .into_any_element(),
            MarkdownModeKind::Preview => preview.into_any_element(),
        };
        div()
            .id("skill-editor")
            .key_context("SkillEditor")
            .track_focus(&self.focus_handle)
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .on_action(cx.listener(|this, _: &SaveSkill, _, cx| this.save(cx)))
            .child(
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
                        Button::new("skill-editor-back")
                            .small()
                            .ghost()
                            .icon(IconName::ArrowLeft)
                            .accessibility_label("返回扩展列表")
                            .tooltip("返回扩展列表，自动保存修改")
                            .disabled(self.closing || (self.pending && !loaded))
                            .on_click(cx.listener(|this, _, _, cx| this.close(cx))),
                    )
                    .child(
                        div()
                            .id("skill-editor-path")
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(path.clone())
                            .tooltip(move |window, cx| {
                                gpui_kit::component::tooltip::Tooltip::new(path.clone())
                                    .build(window, cx)
                            }),
                    )
                    .child(
                        div().flex().items_center().gap_1().children(
                            [
                                (
                                    MarkdownModeKind::editor(),
                                    "skill-mode-editor",
                                    IconName::FilePenLine,
                                    "仅编辑",
                                ),
                                (
                                    MarkdownModeKind::split(),
                                    "skill-mode-split",
                                    IconName::Columns2,
                                    "编辑与预览",
                                ),
                                (
                                    MarkdownModeKind::preview(),
                                    "skill-mode-preview",
                                    IconName::Eye,
                                    "仅预览",
                                ),
                            ]
                            .into_iter()
                            .map(|(mode, id, icon, label)| {
                                Button::new(id)
                                    .small()
                                    .ghost()
                                    .compact()
                                    .icon(icon)
                                    .accessibility_label(label)
                                    .tooltip(label)
                                    .selected(self.mode == mode)
                                    .when(self.mode == mode, |button| {
                                        button
                                            .bg(cx.theme().secondary)
                                            .text_color(cx.theme().secondary_foreground)
                                    })
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.mode = mode;
                                        this.focus(window, cx);
                                        cx.notify();
                                    }))
                            }),
                        ),
                    ),
            )
            .child(div().flex_1().min_h_0().min_w_0().child(content))
            .when_some(self.error.clone(), |view, error| {
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
                    .when(loaded, |view| {
                        view.child(if self.pending {
                            "正在自动保存…"
                        } else if self.error.is_some() {
                            "自动保存失败，修改仍保留在编辑器中"
                        } else if self.dirty {
                            "等待自动保存…"
                        } else {
                            "已保存"
                        })
                    })
                    .when(self.pending && !loaded, |view| view.child("正在读取 Skill…"))
                    .when(loaded && self.dirty && self.error.is_some() && !self.pending, |view| {
                        view.child(
                            Button::new("retry-skill-autosave")
                                .small()
                                .ghost()
                                .icon(IconName::RefreshCw)
                                .accessibility_label("重试自动保存")
                                .tooltip("重试自动保存 · Cmd/Ctrl+S")
                                .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                        )
                    })
                    .when(!loaded && !self.pending, |view| {
                        view.child(
                            Button::new("retry-skill-read")
                                .small()
                                .label("重新读取")
                                .on_click(cx.listener(|this, _, window, cx| this.load(window, cx))),
                        )
                    }),
            )
    }
}

fn preview(content: &str) -> SharedString {
    content
        .strip_prefix("---\n")
        .and_then(|content| content.split_once("\n---\n"))
        .map_or(content, |(_, body)| body)
        .to_owned()
        .into()
}

#[derive(Debug, Error)]
enum EditSkillError {
    #[error("{}: {source}", location.display())]
    Io { location: PathBuf, source: io::Error },
    #[error("Skill 格式无效：{0}")]
    Document(#[from] DocumentError),
    #[error("Skill 名称和说明不能为空")]
    EmptyFrontmatter,
    #[error("文件已被外部修改，未覆盖。请保留草稿并核对磁盘文件。")]
    Conflict,
    #[error("文件为只读，未保存。请检查文件权限。")]
    Readonly,
}

impl EditSkillError {
    fn io(location: &Path, source: io::Error) -> Self {
        Self::Io { location: location.to_owned(), source }
    }

    fn empty_frontmatter() -> Self {
        Self::EmptyFrontmatter
    }
    fn conflict() -> Self {
        Self::Conflict
    }
    fn readonly() -> Self {
        Self::Readonly
    }
}

fn save_skill(location: &Path, original: &str, content: &str) -> Result<(), EditSkillError> {
    let document = FrontmatterDocument::<Frontmatter>::parse(location, content)?;
    if document.frontmatter.name.trim().is_empty()
        || document.frontmatter.description.trim().is_empty()
    {
        return Err(EditSkillError::empty_frontmatter());
    }
    let current =
        fs::read_to_string(location).map_err(|error| EditSkillError::io(location, error))?;
    if current != original {
        return Err(EditSkillError::conflict());
    }
    let permissions =
        fs::metadata(location).map_err(|error| EditSkillError::io(location, error))?.permissions();
    if permissions.readonly() {
        return Err(EditSkillError::readonly());
    }
    let temporary = location.with_extension(format!("{}.tmp", std::process::id()));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file =
        options.open(&temporary).map_err(|error| EditSkillError::io(&temporary, error))?;
    let result = (|| {
        #[cfg(target_os = "macos")]
        fs::copy(location, &temporary).map_err(|error| EditSkillError::io(&temporary, error))?;
        #[cfg(not(target_os = "macos"))]
        file.set_permissions(permissions).map_err(|error| EditSkillError::io(&temporary, error))?;
        file.set_len(0).map_err(|error| EditSkillError::io(&temporary, error))?;
        file.write_all(content.as_bytes())
            .map_err(|error| EditSkillError::io(&temporary, error))?;
        file.sync_all().map_err(|error| EditSkillError::io(&temporary, error))?;
        let current =
            fs::read_to_string(location).map_err(|error| EditSkillError::io(location, error))?;
        if current != original {
            return Err(EditSkillError::conflict());
        }
        fs::rename(&temporary, location).map_err(|error| EditSkillError::io(location, error))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
