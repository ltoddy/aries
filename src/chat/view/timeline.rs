use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::{TextView, TextViewState, TextViewStyle};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::model::chat::Role;
use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;
use crate::{NextUserMessage, PreviousUserMessage};

fn user_message_target(
    anchors: impl DoubleEndedIterator<Item = usize>,
    current: usize,
    inside: bool,
    next: bool,
) -> Option<usize> {
    if next {
        anchors.into_iter().find(|index| *index > current)
    } else {
        anchors.into_iter().rev().find(|index| *index < current || (*index == current && inside))
    }
}

pub fn markdown_style() -> TextViewStyle {
    TextViewStyle { paragraph_gap: rems(0.35), ..Default::default() }
}

impl ChatView {
    fn user_message_target(&self, next: bool) -> Option<usize> {
        let offset = self.timeline.logical_scroll_top();
        let anchors = self
            .chat
            .messages()
            .iter()
            .enumerate()
            .filter_map(|(index, message)| (message.role() == Role::User).then_some(index));
        user_message_target(anchors, offset.item_ix, offset.offset_in_item > px(0.), next)
    }

    pub fn jump_user_message(&mut self, next: bool, cx: &mut Context<Self>) {
        if let Some(item_ix) = self.user_message_target(next) {
            self.timeline.scroll_to(ListOffset { item_ix, offset_in_item: px(0.) });
            cx.notify();
        }
    }

    pub fn render_timeline(&mut self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        if self.chat.messages().is_empty()
            && self.chat.error().is_none()
            && self.loading_session.is_none()
        {
            return div()
                .flex_1()
                .min_h_0()
                .px_6()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_4()
                        .max_w(rems(30.))
                        .text_center()
                        .child(
                            div()
                                .size(rems(3.5))
                                .rounded(rems(1.))
                                .border_1()
                                .border_color(p.border())
                                .bg(p.input())
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(IconName::Bot).size(rems(1.625))),
                        )
                        .child(
                            div()
                                .mt_1()
                                .text_size(rems(1.375))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("开始与 Aries 对话"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .line_height(rems(1.5))
                                .text_color(p.muted())
                                .child("聊聊你的想法，或让 Aries 帮你完成项目中的任务。"),
                        ),
                )
                .into_any_element();
        }
        self.sync_timeline_items();
        let messages = self.chat.messages().len();
        let view = cx.weak_entity();
        let timeline = self.timeline.clone();
        div()
            .id("timeline")
            .flex_1()
            .min_h_0()
            .pl_8()
            .pr_4()
            .py_2()
            .relative()
            .child(
                div().w_full().h_full().child(
                    list(timeline, move |index, window, cx| {
                        view.update(cx, |this, cx| {
                            let gap = match this.chat.messages().get(index).map(|m| m.role()) {
                                Some(Role::User) => rems(0.75),
                                _ => rems(0.25),
                            };
                            div()
                                .w_full()
                                .pb(gap)
                                .child(div().max_w(rems(55.)).mx_auto().child(
                                    if index < messages {
                                        this.timeline_item(index, p, window, cx)
                                    } else {
                                        div()
                                            .id("chat-error")
                                            .rounded(rems(0.75))
                                            .p_2()
                                            .border_1()
                                            .border_color(p.border())
                                            .text_color(p.danger())
                                            .child(this.chat.error().unwrap_or_default().to_owned())
                                            .into_any_element()
                                    },
                                ))
                                .into_any_element()
                        })
                        .unwrap_or_else(|_| div().into_any_element())
                    })
                    .w_full()
                    .h_full(),
                ),
            )
            .child(self.render_user_anchors(p, cx))
            .child(self.render_scroll_controls(p, cx))
            .into_any_element()
    }

    fn render_scroll_controls(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        div()
            .absolute()
            .bottom_3()
            .right_5()
            .flex()
            .flex_col()
            .items_end()
            .gap_2()
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        Button::new("scroll-to-top")
                            .outline()
                            .small()
                            .bg(p.input())
                            .icon(IconName::ArrowUpToLine)
                            .accessibility_label("回到顶部")
                            .tooltip("回到顶部")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.timeline.scroll_to(ListOffset::default());
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("previous-user-message")
                            .outline()
                            .small()
                            .bg(p.input())
                            .icon(IconName::ArrowUp)
                            .accessibility_label("上一条用户消息")
                            .tooltip_with_action("上一条用户消息", &PreviousUserMessage, None)
                            .disabled(self.user_message_target(false).is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.jump_user_message(false, cx);
                            })),
                    )
                    .child(
                        Button::new("next-user-message")
                            .outline()
                            .small()
                            .bg(p.input())
                            .icon(IconName::ArrowDown)
                            .accessibility_label("下一条用户消息")
                            .tooltip_with_action("下一条用户消息", &NextUserMessage, None)
                            .disabled(self.user_message_target(true).is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.jump_user_message(true, cx);
                            })),
                    ),
            )
            .when(!self.timeline.is_following_tail(), |controls| {
                controls.child(
                    Button::new("scroll-to-bottom")
                        .outline()
                        .small()
                        .bg(p.input())
                        .icon(IconName::ArrowDown)
                        .label("回到最新")
                        .accessibility_label("回到最新消息并恢复自动跟随")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.timeline.set_follow_mode(FollowMode::Tail);
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }

    fn render_user_anchors(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let timeline = self.timeline.clone();
        let view = cx.weak_entity();
        let anchors = self
            .chat
            .messages()
            .iter()
            .enumerate()
            .filter(|(_, message)| matches!(message.role(), Role::User))
            .map(|(index, message)| (index, message.text()));
        if anchors.clone().next().is_none() {
            return div().into_any_element();
        }
        div()
            .id("user-anchors")
            .absolute()
            .left(rems(0.25))
            .top_0()
            .bottom_0()
            .w_6()
            .py_3()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .overflow_y_scroll()
            .children(anchors.enumerate().map(|(anchor_index, (index, text))| {
                let timeline = timeline.clone();
                let view = view.clone();
                let preview = text.chars().take(300).collect::<String>();
                let group = SharedString::from(format!("user-anchor-{index}"));
                Button::new(("user-anchor", index))
                    .ghost()
                    .small()
                    .group(group.clone())
                    .size_6()
                    .p_0()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .accessibility_label(format!("跳转到第 {} 条用户消息", anchor_index + 1))
                    .tooltip_show_delay(Duration::from_millis(250))
                    .tooltip(format!(
                        "{preview}\n{} + Alt + ↑ / ↓ 跳转用户消息",
                        if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" }
                    ))
                    .on_click(move |_, _, cx| {
                        timeline.scroll_to(ListOffset { item_ix: index, offset_in_item: px(0.) });
                        let _ = view.update(cx, |_, cx| cx.notify());
                    })
                    .child(
                        div()
                            .w(rems(0.75))
                            .h(rems(0.125))
                            .rounded_full()
                            .bg(p.border())
                            .group_hover(group, |style| style.bg(p.foreground())),
                    )
            }))
            .into_any_element()
    }

    fn sync_timeline_items(&self) {
        let count = self.chat.messages().len() + usize::from(self.chat.error().is_some());
        let current = self.timeline.item_count();
        if current < count {
            self.timeline.splice(current..current, count - current);
        } else if current > count {
            self.timeline.splice(count..current, 0);
        }
    }

    pub fn markdown_state(
        &mut self,
        index: usize,
        cx: &mut Context<Self>,
    ) -> Entity<TextViewState> {
        let text = self.chat.messages()[index].text();
        let value = if text.is_empty() { "（空内容）" } else { text };
        let state = self
            .markdown
            .entry(index)
            .or_insert_with(|| cx.new(|cx| TextViewState::markdown(value, cx)))
            .clone();
        state.update(cx, |state, cx| state.set_text(value, cx));
        state
    }

    fn timeline_item(
        &mut self,
        index: usize,
        p: Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        self.cache_visible_images(index);
        if self.chat.messages()[index].role() == Role::Thought {
            return self.render_thought_message(index, p, cx);
        }
        let message = &self.chat.messages()[index];
        if let Some(subagent) = message.subagent() {
            return self.render_subagent_message(subagent, p, cx);
        }
        if let Some(tool) = message.tool_call() {
            return self.render_tool_message(index, tool, p, window, cx);
        }
        if message.role() == Role::User {
            return self.render_user_message(index, message, p);
        }
        let markdown = self.markdown_state(index, cx);
        let message = &self.chat.messages()[index];
        let card = message.role() == Role::Event;
        let body = div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .when(card, |body| {
                body.px_2()
                    .py_1()
                    .rounded(rems(0.625))
                    .bg(p.input())
                    .border_1()
                    .border_color(p.border())
            })
            .when(card || !message.title().is_empty(), |body| {
                body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(p.muted())
                        .when(card, |header| {
                            header.child(Icon::new(IconName::Activity).size(rems(0.875)))
                        })
                        .child(message.title().to_owned()),
                )
            })
            .child(
                TextView::new(&markdown)
                    .style(markdown_style())
                    .selectable(true)
                    .scrollable(false)
                    .w_full()
                    .code_block_actions(|code_block, _window, cx| {
                        let code = code_block.code().to_string();
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when_some(code_block.lang(), |el, lang| {
                                el.child(
                                    div()
                                        .text_xs()
                                        .text_color(cx.theme().muted)
                                        .child(lang.to_string()),
                                )
                            })
                            .child({
                                Button::new("copy-code-block")
                                    .ghost()
                                    .xsmall()
                                    .on_click(move |_, _, cx| {
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            code.clone(),
                                        ));
                                    })
                                    .child(
                                        Icon::new(IconName::Copy)
                                            .xsmall()
                                            .text_color(cx.theme().muted),
                                    )
                            })
                            .into_any_element()
                    })
                    .text_sm()
                    .line_height(rems(1.5))
                    .into_any_element(),
            )
            .when_some(self.images.get(&index), |body, images| {
                body.child(div().id(("images", index)).w_full().flex().flex_col().gap_1().children(
                    images.iter().map(|image| {
                        img(image.clone())
                            .w_full()
                            .h(rems(12.5))
                            .object_fit(ObjectFit::Contain)
                            .with_fallback(|| div().child("图片无法预览").into_any_element())
                    }),
                ))
            });
        div()
            .id(("message", index))
            .aria_label(message.title().to_owned())
            .aria_value(message.text().to_owned())
            .w_full()
            .flex()
            .child(div().min_w_0().w_full().child(body))
            .into_any_element()
    }
}
