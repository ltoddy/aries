use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Enter, Escape, IndentInline, MoveDown, MoveUp, Textarea};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::commands::CommandMenuAction;
use crate::chat::view::palette::Palette;

impl ChatView {
    pub fn composer(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let can_send = self.can_send(cx);
        div().flex_shrink_0().pl_8().pr_4().py_2().bg(p.background()).child(
            div()
                .w_full()
                .max_w(rems(55.))
                .mx_auto()
                .flex()
                .flex_col()
                .gap_2()
                .when_some(self.permissions.front(), |composer, permission| {
                    composer.child(
                        div()
                            .p_3()
                            .rounded(cx.theme().radius_lg)
                            .border_1()
                            .border_color(p.border())
                            .bg(p.input())
                            .child(
                                div()
                                    .mb_2()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(permission.title().to_owned()),
                            )
                            .child(div().flex().flex_wrap().gap_2().children(
                                permission.options().iter().enumerate().map(|(index, option)| {
                                    let id = option.option_id.clone();
                                    Button::new(("permission", index))
                                        .label(option.name.clone())
                                        .small()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_permission(id.clone(), cx)
                                        }))
                                }),
                            )),
                    )
                })
                .child(self.render_execution_context(p))
                .child(
                    div()
                        .relative()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .capture_action(cx.listener(|this, _: &MoveUp, window, cx| {
                            this.command_menu_action(CommandMenuAction::Previous, window, cx)
                        }))
                        .capture_action(cx.listener(|this, _: &MoveDown, window, cx| {
                            this.command_menu_action(CommandMenuAction::Next, window, cx)
                        }))
                        .capture_action(cx.listener(|this, action: &Enter, window, cx| {
                            if !action.shift && !action.secondary {
                                this.command_menu_action(CommandMenuAction::Complete, window, cx);
                            }
                        }))
                        .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                            this.command_menu_action(CommandMenuAction::Complete, window, cx)
                        }))
                        .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                            this.command_menu_action(CommandMenuAction::Dismiss, window, cx)
                        }))
                        .children(self.render_commands(p, cx))
                        .child(
                            div()
                                .flex()
                                .items_end()
                                .gap_2()
                                .p_1()
                                .rounded(rems(0.875))
                                .border_1()
                                .border_color(p.border())
                                .bg(p.input())
                                .child(
                                    div().id("composer-input").flex_1().min_w_0().px_1().child(
                                        Textarea::new(&self.input)
                                            .appearance(false)
                                            .bordered(false)
                                            .aria_label("消息")
                                            .w_full(),
                                    ),
                                )
                                .child(
                                    Button::new("send")
                                        .primary()
                                        .icon(
                                            Icon::new(if self.chat.is_busy() {
                                                IconName::Square
                                            } else {
                                                IconName::Send
                                            })
                                            .size(rems(1.0625)),
                                        )
                                        .accessibility_label(if self.chat.is_busy() {
                                            "停止回复"
                                        } else {
                                            "发送消息"
                                        })
                                        .size_10()
                                        .rounded(cx.theme().radius_lg)
                                        .disabled(if self.chat.is_busy() {
                                            self.stopping
                                        } else {
                                            !can_send
                                        })
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            if this.chat.is_busy() {
                                                this.stop(window, cx);
                                            } else {
                                                this.send(window, cx);
                                            }
                                        })),
                                ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .when(
                            !self.chat.config_options().is_empty() || self.config_error.is_some(),
                            |footer| footer.child(self.render_session_config(p, cx)),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .min_h_6()
                                .text_xs()
                                .text_color(p.muted())
                                .child(
                                    div()
                                        .id("usage")
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .when_some(self.tokens, |usage, tokens| {
                                            usage.child(format!("{tokens} tokens"))
                                        }),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_right()
                                        .child("Enter 发送 · Shift + Enter 换行"),
                                ),
                        ),
                ),
        )
    }
}
