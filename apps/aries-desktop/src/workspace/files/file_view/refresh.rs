use gpui_kit::component::input::EditorState;
use gpui_kit::{AppContext, Context, SharedString, Window};

use crate::workspace::files::file_view::document::{
    FileContent, ReadFileError, read_file_with_identity,
};
use crate::workspace::files::file_view::status::{EditorStatus, file_language};
use crate::workspace::files::file_view::{FileLoadState, FileView};

impl FileView {
    pub fn refresh_from_disk(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.refresh_revision = self.refresh_revision.wrapping_add(1);
        self.auto_save_task = None;
        if self.saving || self.is_loading() || self.refresh_task.is_some() {
            self.refresh_pending = true;
            return;
        }
        let Some(target) = self.target.as_ref() else {
            return;
        };
        let path = target.path().to_owned();
        let revision = self.revision;
        let refresh_revision = self.refresh_revision;
        self.refresh_pending = false;
        let read_path = path.clone();
        let task = cx.background_spawn(async move { read_file_with_identity(&read_path) });
        self.refresh_task = Some(cx.spawn_in(window, async move |this, cx| {
            let (identity, result) = task.await;
            let _ = this.update_in(cx, |this, window, cx| {
                if this.revision != revision
                    || this.target.as_ref().is_none_or(|target| target.path() != path)
                {
                    return;
                }
                this.refresh_task = None;
                if this.saving {
                    this.refresh_pending = true;
                    return;
                }
                if this.refresh_revision != refresh_revision {
                    this.refresh_from_disk(window, cx);
                    return;
                }
                if identity.is_some()
                    && this.watch_identity.as_ref().is_some_and(|watched| watched != &identity)
                {
                    this.start_watch(window, cx);
                }
                if this.is_ready() {
                    this.dirty = this
                        .document
                        .as_ref()
                        .is_some_and(|document| document.content != this.editor.read(cx).value());
                }
                match result {
                    Ok(FileContent::Text(document)) => {
                        this.refresh_error = None;
                        let disk_changed =
                            this.document.as_ref().is_none_or(|old| !old.same_original(&document));
                        if disk_changed || !this.is_ready() || !this.dirty {
                            let content = this.editor.read(cx).value();
                            if content != document.content {
                                this.editor.update(cx, |editor, cx| {
                                    update_editor(editor, document.content.clone(), window, cx);
                                });
                            }
                            if !this.is_ready() {
                                let language = this
                                    .target
                                    .as_ref()
                                    .map_or("plaintext", |target| file_language(target.path()));
                                this.editor.update(cx, |editor, cx| {
                                    editor.set_highlighter(language, cx);
                                    editor.set_readonly(false, cx);
                                });
                            }
                            this.dirty = false;
                            this.save_error = None;
                            this.feedback = None;
                        }
                        this.document = Some(document);
                        this.image = None;
                        this.state = FileLoadState::text();
                        this.status = EditorStatus::new(this.editor.read(cx));
                    },
                    Ok(FileContent::Image(document)) => {
                        this.refresh_error = None;
                        this.editor.update(cx, |editor, cx| {
                            editor.set_readonly(true, cx);
                            editor.set_value("", window, cx);
                        });
                        this.dirty = false;
                        this.save_error = None;
                        this.feedback = None;
                        this.document = None;
                        this.image = Some(document);
                        this.state = FileLoadState::image();
                    },
                    Err(error)
                        if matches!(&error, ReadFileError::Io(source) if source.kind() != std::io::ErrorKind::NotFound)
                            && (this.is_ready() || this.is_image()) =>
                    {
                        this.refresh_error = Some(error.to_string());
                    },
                    Err(error) => {
                        this.refresh_error = None;
                        this.editor.update(cx, |editor, cx| {
                            editor.set_readonly(true, cx);
                            editor.set_value("", window, cx);
                        });
                        this.dirty = false;
                        this.save_error = None;
                        this.feedback = None;
                        this.document = None;
                        this.image = None;
                        this.state = FileLoadState::failed(error);
                    },
                }
                if this.refresh_pending {
                    this.refresh_from_disk(window, cx);
                } else if this.save_error.is_none() {
                    this.schedule_auto_save(window, cx);
                }
                cx.notify();
            });
        }));
    }
}

fn update_editor(
    editor: &mut EditorState,
    content: SharedString,
    window: &mut Window,
    cx: &mut Context<EditorState>,
) {
    let selection = editor.selected_range();
    let reversed = editor.is_selection_reversed();
    let offset = editor.scroll_offset();
    let y = editor.scroll_y_f64();
    let match_ix = editor.search_session().matcher.current_match_index();
    editor.set_value(content, window, cx);
    if editor.search_session().is_active() {
        let last_match = editor.search_session().matcher.len().saturating_sub(1);
        for _ in 0..match_ix.min(last_match) {
            let _ = editor.next_search_match(cx);
        }
    }
    editor.set_selected_range_with_direction(selection, reversed, cx);
    editor.set_scroll_offset(offset, cx);
    editor.set_scroll_y_f64(y, cx);
}
