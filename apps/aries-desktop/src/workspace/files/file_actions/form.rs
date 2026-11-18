use gpui_kit::base::actions::Confirm;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::input::Input;
use gpui_kit::component::{ActiveTheme, Disableable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::files::file_actions::{FileActions, validate_basename};

impl FileActions {
    pub fn render_dialog(
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
                let consequence = if form.prepared.as_ref().and_then(|prepared| prepared.entry_metadata())
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
