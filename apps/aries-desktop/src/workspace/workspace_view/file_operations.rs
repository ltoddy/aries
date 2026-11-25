use std::collections::VecDeque;
use std::time::Duration;

use gpui_kit::base::actions::Confirm;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::{Dialog, DialogClose, DialogEntrance, DialogFooter};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::{ActiveTheme, Disableable, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::files::FileView;
use crate::workspace::workspace_view::{
    FileConfirmation, PendingFileOperation, WorkspaceTab, WorkspaceView,
};

impl WorkspaceView {
    pub fn request_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.files.confirmation.is_some() || self.files.pending.is_empty() {
            return;
        }
        self.files.focus = window.focused(cx);
        self.files.discarded.clear();
        self.files.pending.push_back(PendingFileOperation::quit());
        self.pause_file_operation_auto_save(cx);
        self.advance_file_operations(window, cx);
    }

    pub fn request_file_operations(
        &mut self,
        operations: VecDeque<PendingFileOperation>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.files.pending.is_empty() || !self.can_navigate(window, cx) {
            return;
        }
        self.files.focus = window.focused(cx);
        self.files.pending = operations;
        self.pause_file_operation_auto_save(cx);
        self.advance_file_operations(window, cx);
    }

    pub fn pause_file_operation_auto_save(&self, cx: &mut Context<Self>) {
        let quitting = self
            .files
            .pending
            .iter()
            .any(|operation| matches!(operation, PendingFileOperation::Quit));
        for tab in &self.tabs.items {
            if (quitting
                || self.files.pending.iter().any(|operation| operation.tab_id() == Some(tab.id())))
                && let Some(view) = tab.file()
            {
                view.update(cx, |view, _| view.pause_auto_save());
            }
        }
    }

    pub fn resume_file_auto_save(&self, window: &mut Window, cx: &mut Context<Self>) {
        for view in self.tabs.items.iter().filter_map(WorkspaceTab::file) {
            view.update(cx, |view, cx| view.resume_auto_save(window, cx));
        }
    }

    pub fn advance_file_operations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.files.confirmation.is_some() {
            return;
        }
        while let Some(operation) = self.files.pending.front().copied() {
            if matches!(operation, PendingFileOperation::Quit)
                && self.tabs.items.iter().all(|tab| {
                    self.files.discarded.contains(&tab.id())
                        || tab.file().is_none_or(|view| {
                            !view.read(cx).is_dirty() && !view.read(cx).is_saving()
                        })
                })
            {
                self.files.pending.clear();
                self.files.wait = None;
                cx.quit();
                return;
            }
            if window.has_active_dialog(cx)
                || !self.file_tree.update(cx, |tree, cx| tree.cancel_actions(window, cx))
            {
                self.wait_for_file_operations(window, cx);
                return;
            }
            self.files.wait = None;
            if let Some(id) = operation.tab_id() {
                if let Some(view) = self.file_view(id).cloned() {
                    view.update(cx, |view, _| view.pause_auto_save());
                    if view.read(cx).is_saving() {
                        self.wait_for_file_operations(window, cx);
                        return;
                    }
                    let reload = matches!(operation, PendingFileOperation::Reload(_));
                    if !reload && view.read(cx).is_dirty() && view.read(cx).save_error().is_none() {
                        view.update(cx, |view, cx| view.save(window, cx));
                        self.wait_for_file_operations(window, cx);
                        return;
                    }
                    if !reload && !view.read(cx).is_dirty() {
                        self.files.pending.pop_front();
                        self.apply_file_operation(operation, window, cx);
                        continue;
                    }
                    self.files.confirmation = Some(FileConfirmation::new(id, false));
                    self.invalidate_file_navigation(cx);
                    let owner = cx.weak_entity();
                    window.open_dialog(cx, move |dialog, _, cx| {
                        let Some(entity) = owner.upgrade() else {
                            return dialog.on_ok(|_, _, _| false);
                        };
                        entity.read(cx).render_file_confirmation(dialog, id, owner.clone(), cx)
                    });
                    if let Some(confirmation) = self.files.confirmation.as_mut() {
                        confirmation.focus = window.focused(cx);
                    }
                    cx.notify();
                    return;
                }
                self.files.pending.pop_front();
                self.apply_file_operation(operation, window, cx);
            } else {
                let dirty = self.tabs.items.iter().find(|tab| {
                    !self.files.discarded.contains(&tab.id())
                        && tab.file().is_some_and(|view| {
                            view.read(cx).is_dirty() || view.read(cx).is_saving()
                        })
                });
                if let Some(tab) = dirty {
                    self.files.pending.push_front(PendingFileOperation::quit_file(tab.id()));
                }
            }
        }
        self.files.focus = None;
        self.files.discarded.clear();
        self.files.wait = None;
        self.resume_file_auto_save(window, cx);
        self.focus_active(window, cx);
        cx.notify();
    }

    pub fn wait_for_file_operations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.files.wait.is_some() {
            return;
        }
        self.files.wait = Some(cx.spawn_in(window, async move |owner, cx| {
            cx.background_executor().timer(Duration::from_millis(100)).await;
            let _ = owner.update_in(cx, |this, window, cx| {
                this.files.wait = None;
                this.advance_file_operations(window, cx);
            });
        }));
    }

    pub fn apply_file_operation(
        &mut self,
        operation: PendingFileOperation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match operation {
            PendingFileOperation::Close(id) => self.remove_tab(id, cx),
            PendingFileOperation::Reload(id) => {
                if let Some(view) = self.file_view(id) {
                    view.update(cx, |view, cx| {
                        view.reload_discarding(window, cx);
                        view.resume_auto_save(window, cx);
                    });
                }
                self.resolve_file_identity(id, window, cx);
            },
            PendingFileOperation::QuitFile(_) => {},
            PendingFileOperation::Quit => {},
        }
    }

    pub fn confirmation_is_busy(&self, id: usize, cx: &App) -> bool {
        self.files
            .confirmation
            .as_ref()
            .is_none_or(|confirmation| confirmation.id != id || confirmation.saving_requested)
            || self.file_view(id).is_some_and(|view| view.read(cx).is_saving())
    }

    pub fn save_file_confirmation(
        &mut self,
        id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.confirmation_is_busy(id, cx) {
            return;
        }
        let Some(view) = self.file_view(id).cloned() else {
            return;
        };
        if matches!(self.files.pending.front(), Some(PendingFileOperation::Reload(_)))
            && (!view.read(cx).is_dirty() || view.read(cx).save_error().is_none())
        {
            self.discard_file_confirmation(id, window, cx);
            return;
        }
        if !view.read(cx).is_ready()
            || !view.read(cx).is_dirty()
            || view.read(cx).save_error().is_none()
        {
            return;
        }
        view.update(cx, |view, cx| {
            view.pause_auto_save();
            view.save(window, cx);
        });
        if let Some(confirmation) = self.files.confirmation.as_mut() {
            confirmation.saving_requested = view.read(cx).is_saving();
        }
        window.refresh();
        cx.notify();
    }

    pub fn owns_file_confirmation(&self, id: usize, window: &Window, cx: &App) -> bool {
        self.files.confirmation.as_ref().is_some_and(|confirmation| {
            confirmation.id == id
                && confirmation.focus.as_ref().is_some_and(|focus| {
                    focus.is_focused(window) || focus.contains_focused(window, cx)
                })
        })
    }

    pub fn discard_file_confirmation(
        &mut self,
        id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.confirmation_is_busy(id, cx) || !self.owns_file_confirmation(id, window, cx) {
            return;
        }
        if matches!(self.files.pending.front(), Some(PendingFileOperation::QuitFile(_))) {
            self.files.discarded.insert(id);
        }
        self.finish_file_confirmation(id, window, cx);
    }

    pub fn finish_file_confirmation(
        &mut self,
        id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.owns_file_confirmation(id, window, cx) {
            return;
        }
        self.files.confirmation = None;
        window.close_dialog(cx);
        if let Some(operation) = self.files.pending.pop_front() {
            self.apply_file_operation(operation, window, cx);
        }
        cx.defer_in(window, |this, window, cx| this.advance_file_operations(window, cx));
        cx.notify();
    }

    pub fn cancel_file_confirmation(
        &mut self,
        id: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.confirmation_is_busy(id, cx) {
            return false;
        }
        self.files.confirmation = None;
        self.files.pending.clear();
        self.files.discarded.clear();
        self.files.wait = None;
        cx.defer_in(window, |this, window, cx| {
            if !this.files.pending.is_empty() {
                this.resume_file_auto_save(window, cx);
            }
        });
        if let Some(focus) = self.files.focus.take() {
            window.defer(cx, move |window, cx| focus.focus(window, cx));
        }
        cx.notify();
        true
    }

    pub fn render_file_confirmation(
        &self,
        dialog: Dialog,
        id: usize,
        owner: WeakEntity<Self>,
        cx: &App,
    ) -> Dialog {
        let busy = self.confirmation_is_busy(id, cx);
        let view = self.file_view(id).map(|view| view.read(cx));
        let target = view.and_then(FileView::target);
        let name = target.map(|target| {
            target.path().file_name().unwrap_or(target.path().as_os_str()).to_string_lossy()
        });
        let reload = matches!(self.files.pending.front(), Some(PendingFileOperation::Reload(_)));
        let title = if reload {
            format!("重新读取“{}”？", name.unwrap_or_default())
        } else if busy {
            format!("正在保存“{}”", name.unwrap_or_default())
        } else {
            format!("“{}”自动保存失败", name.unwrap_or_default())
        };
        let save = owner.clone();
        let cancel = owner.clone();
        let discard = owner.clone();
        let enter = owner.clone();
        let confirm = owner.clone();
        let dirty = view.is_some_and(FileView::is_dirty);
        let retry = busy || view.is_some_and(|view| view.is_dirty() && view.save_error().is_some());
        let disabled_save = busy
            || view.is_none_or(|view| {
                if reload && !retry { !view.is_open() } else { !view.is_ready() || !dirty }
            });
        dialog
            .title(title)
            .w(cx.theme().font_size * 28.)
            .entrance(DialogEntrance::None)
            .overlay_closable(false)
            .close_button(false)
            .on_ok(move |_, window, cx| {
                let _ = save.update(cx, |this, cx| this.save_file_confirmation(id, window, cx));
                false
            })
            .on_cancel(move |_, window, cx| {
                cancel
                    .update(cx, |this, cx| this.cancel_file_confirmation(id, window, cx))
                    .unwrap_or(false)
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .min_w_0()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .whitespace_normal()
                            .child(
                                target
                                    .map(|target| target.path().display().to_string())
                                    .unwrap_or_default(),
                            ),
                    )
                    .child(if reload {
                        "自动保存已暂停。重新读取磁盘内容将丢弃未保存的修改，无法撤销。"
                    } else if busy {
                        "正在重试保存，草稿仍保留在当前标签中。"
                    } else {
                        "自动保存失败，草稿仍保留在当前标签中。可以重试保存；放弃修改后无法恢复。"
                    })
                    .when(busy, |body| {
                        body.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(Spinner::new().small())
                                .child("正在保存，请稍候"),
                        )
                    })
                    .when_some(view.and_then(FileView::save_error), |body, error| {
                        body.child(
                            div().text_sm().text_color(cx.theme().danger).child(error.to_owned()),
                        )
                    }),
            )
            .footer(
                div()
                    .capture_key_down(move |event, window, cx| {
                        if event.keystroke.key == "enter"
                            && event.keystroke.modifiers == Modifiers::default()
                        {
                            window.prevent_default();
                            cx.stop_propagation();
                            let _ = enter
                                .update(cx, |this, cx| this.save_file_confirmation(id, window, cx));
                        }
                    })
                    .capture_action(move |_: &Confirm, window, cx| {
                        cx.stop_propagation();
                        let _ = confirm
                            .update(cx, |this, cx| this.save_file_confirmation(id, window, cx));
                    })
                    .child(
                        DialogFooter::new()
                            .when(dirty && (!reload || retry), |footer| {
                                footer.child(
                                    Button::new(("discard-file-changes", id))
                                        .danger()
                                        .label("放弃修改")
                                        .disabled(busy)
                                        .on_click(move |_, window, cx| {
                                            let _ = discard.update(cx, |this, cx| {
                                                this.discard_file_confirmation(id, window, cx);
                                            });
                                        }),
                                )
                            })
                            .child(div().flex_shrink_0().child(DialogClose::new().trigger(
                                |button| {
                                    button.label("取消").accessibility_label("取消").disabled(busy)
                                },
                            )))
                            .when(retry || reload, |footer| {
                                footer.child(
                                    Button::new(("save-file-changes", id))
                                        .primary()
                                        .when(reload && !retry && dirty, |button| button.danger())
                                        .label(if retry {
                                            "重试保存"
                                        } else if dirty {
                                            "放弃修改"
                                        } else {
                                            "重新读取"
                                        })
                                        .disabled(disabled_save)
                                        .on_click(move |_, window, cx| {
                                            let _ = owner.update(cx, |this, cx| {
                                                this.save_file_confirmation(id, window, cx)
                                            });
                                        }),
                                )
                            }),
                    ),
            )
    }
}
