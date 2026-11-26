use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Editor, EditorState, InputEvent, RopeExt};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;
use url::Url;

use crate::workspace::files::file_view::document::{FileContent, read_file, save_document};
use crate::workspace::files::file_view::status::{EditorStatus, editor_tab_size, file_language};
use crate::workspace::files::file_view::{AUTO_SAVE_DELAY, FileLoadState, FileView, OpenFile};
use crate::workspace::settings::typography;

impl FileView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| {
            let tab = editor_tab_size();
            EditorState::new(window, cx)
                .soft_wrap(false)
                .line_number(true)
                .tab_size(tab)
                .replaceable(true)
                .scroll_beyond_last_line(Some(0))
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
            image: None,
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
        self.image = None;
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
        let task = cx.background_spawn(async move { read_file(&path) });
        self.load_task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| {
                this.update(cx, |this, cx| {
                    if this.revision != revision || !this.is_open() {
                        return;
                    }
                    match result {
                        Ok(FileContent::Text(document)) => {
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
                            this.state = FileLoadState::text();
                        },
                        Ok(FileContent::Image(document)) => {
                            this.image = Some(document);
                            this.state = FileLoadState::image();
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
        self.state.is_text()
    }

    pub fn is_image(&self) -> bool {
        self.state.is_image()
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.state, FileLoadState::Loading)
    }

    pub fn has_error(&self) -> bool {
        matches!(self.state, FileLoadState::Failed(_))
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.is_text() {
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
            && self.state.is_text()
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
        if !self.is_open() || !self.state.is_text() {
            return;
        }
        self.editor.update(cx, |editor, cx| {
            editor.focus(window, cx);
            editor.open_search(false, cx);
        });
    }

    pub fn copy_all(&mut self, cx: &mut Context<Self>) {
        if !self.is_open() || !self.state.is_text() {
            return;
        }
        let text = self.editor.read(cx).text().to_string();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.feedback = Some("已复制全文".into());
        cx.notify();
    }

    pub fn navigate_match(&mut self, previous: bool, cx: &mut Context<Self>) {
        if !self.state.is_text() || !self.editor.read(cx).search_session().is_active() {
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

    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editor.read(cx).search_session().open {
            self.editor.update(cx, |editor, cx| {
                editor.close_search(cx);
                editor.focus(window, cx);
            });
        }
    }

    pub fn open_externally(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.target.as_ref() else {
            return;
        };
        let Ok(url) = Url::from_file_path(target.path()) else {
            self.feedback = Some("无法生成文件链接，未打开外部应用".into());
            cx.notify();
            return;
        };
        cx.open_url(url.as_str());
    }

    pub fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    pub fn render_content(&self, cx: &mut Context<Self>) -> AnyElement {
        match &self.state {
            FileLoadState::Text => Editor::new(&self.editor)
                .appearance(false)
                .bordered(false)
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(typography::code_size())
                .aria_label("文件内容，编辑后自动保存")
                .h_full()
                .w_full()
                .into_any_element(),
            FileLoadState::Image => {
                let Some(image) = self.image.as_ref() else {
                    return Empty.into_any_element();
                };
                let danger = cx.theme().danger;
                div()
                    .size_full()
                    .min_h_0()
                    .p_4()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        img(image.image.clone())
                            .max_w_full()
                            .max_h_full()
                            .object_fit(ObjectFit::Contain)
                            .with_fallback(move || {
                                div()
                                    .text_color(danger)
                                    .child("无法解码这张图片")
                                    .into_any_element()
                            }),
                    )
                    .into_any_element()
            },
            FileLoadState::Loading => div()
                .flex()
                .items_center()
                .gap_2()
                .p_3()
                .text_color(cx.theme().muted_foreground)
                .child(Spinner::new().small())
                .child("正在读取文件…")
                .into_any_element(),
            FileLoadState::Failed(error) => {
                let mut actions = div().flex().gap_2();
                if error.can_open_externally() {
                    actions = actions.child(
                        Button::new("file-view-open-external")
                            .small()
                            .label("用默认应用打开")
                            .on_click(cx.listener(|this, _, _, cx| this.open_externally(cx))),
                    );
                }
                if error.is_retryable() {
                    actions = actions.child(
                        Button::new("file-view-retry")
                            .small()
                            .label("重新读取")
                            .on_click(cx.listener(|this, _, window, cx| this.reload(window, cx))),
                    );
                }
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .child(div().text_color(cx.theme().danger).child(error.to_string()))
                    .child(actions)
                    .into_any_element()
            },
            FileLoadState::Idle => div()
                .p_3()
                .text_color(cx.theme().muted_foreground)
                .child("从文件树选择文件以编辑内容")
                .into_any_element(),
        }
    }
}
