use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::text::{TextView, TextViewState, TextViewStyle};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::session::chat::Role;
use crate::ui::ChatView;
use crate::ui::palette::Palette;

pub(crate) fn markdown_style() -> TextViewStyle {
    TextViewStyle { paragraph_gap: rems(0.35), ..Default::default() }
}

impl ChatView {
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
                        .max_w(px(480.))
                        .text_center()
                        .child(
                            div()
                                .size(px(56.))
                                .rounded(px(16.))
                                .border_1()
                                .border_color(p.border())
                                .bg(p.input())
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(IconName::Bot).size(px(26.))),
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
        let error = self.chat.error().map(str::to_owned);
        let view = cx.weak_entity();
        let timeline = self.timeline.clone();
        div()
            .id("timeline")
            .flex_1()
            .min_h_0()
            .pl_5()
            .pr_4()
            .py_2()
            .relative()
            .child(
                div().max_w(px(880.)).w_full().h_full().mx_auto().child(
                    list(timeline, move |index, window, cx| {
                        let view = view.clone();
                        view.update(cx, |this, cx| {
                            let gap = match this.chat.messages().get(index).map(|m| m.role()) {
                                Some(Role::User) => px(12.),
                                _ => px(4.),
                            };
                            div()
                                .w_full()
                                .pb(gap)
                                .child(if index < messages {
                                    this.timeline_item(index, p, window, cx)
                                } else {
                                    div()
                                        .id("chat-error")
                                        .rounded(px(12.))
                                        .p_2()
                                        .border_1()
                                        .border_color(p.border())
                                        .text_color(p.danger())
                                        .child(error.clone().unwrap_or_default())
                                        .into_any_element()
                                })
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
        let timeline = self.timeline.clone();
        let top_timeline = timeline.clone();
        let view = cx.weak_entity();
        let top_view = view.clone();
        div()
            .absolute()
            .bottom(px(12.))
            .right(px(20.))
            .flex()
            .flex_col()
            .gap_1()
            .child(Self::scroll_control(
                "scroll-to-top",
                IconName::ArrowUp,
                "滚动到顶部",
                p,
                move |cx| {
                    top_timeline.scroll_to(ListOffset { item_ix: 0, offset_in_item: px(0.) });
                    let _ = top_view.update(cx, |_, cx| cx.notify());
                },
            ))
            .child(Self::scroll_control(
                "scroll-to-bottom",
                IconName::ArrowDown,
                "滚动到底部",
                p,
                move |cx| {
                    timeline.scroll_to_end();
                    let _ = view.update(cx, |_, cx| cx.notify());
                },
            ))
            .into_any_element()
    }

    fn scroll_control(
        id: &'static str,
        icon: IconName,
        label: &'static str,
        p: Palette,
        on_click: impl Fn(&mut App) + 'static,
    ) -> AnyElement {
        div()
            .id(id)
            .size_8()
            .rounded_full()
            .border_1()
            .border_color(p.border())
            .bg(p.input())
            .flex()
            .items_center()
            .justify_center()
            .aria_label(label)
            .hover(|style| style.bg(p.muted()))
            .tooltip(move |window, cx| {
                Tooltip::element(move |_, _| div().child(label)).build(window, cx)
            })
            .on_click(move |_, _, cx| on_click(cx))
            .child(Icon::new(icon).size(px(16.)).text_color(p.foreground()))
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
            .map(|(index, message)| (index, message.text().to_owned()))
            .collect::<Vec<_>>();
        if anchors.is_empty() {
            return div().into_any_element();
        }
        div()
            .id("user-anchors")
            .absolute()
            .left(px(4.))
            .top_0()
            .bottom_0()
            .w(px(16.))
            .py_3()
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .overflow_y_scroll()
            .children(anchors.into_iter().enumerate().map(|(anchor_index, (index, text))| {
                let timeline = timeline.clone();
                let view = view.clone();
                let preview = text.chars().take(300).collect::<String>();
                let group = SharedString::from(format!("user-anchor-{index}"));
                div()
                    .id(("user-anchor", index))
                    .group(group.clone())
                    .w(px(16.))
                    .h(px(10.))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .aria_label(format!("跳转到第 {} 条用户消息", anchor_index + 1))
                    .tooltip_show_delay(Duration::ZERO)
                    .tooltip(move |window, cx| {
                        let preview = preview.clone();
                        Tooltip::element(move |_, _| {
                            div()
                                .max_w(px(360.))
                                .whitespace_normal()
                                .line_height(rems(1.25))
                                .child(preview.clone())
                        })
                        .build(window, cx)
                    })
                    .on_click(move |_, _, cx| {
                        timeline.scroll_to_reveal_item(index);
                        let _ = view.update(cx, |_, cx| cx.notify());
                    })
                    .child(
                        div()
                            .w(px(12.))
                            .h(px(2.))
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
        if current != count {
            self.timeline.reset_with_uniform_height(count, px(96.));
        }
    }

    pub(crate) fn markdown_state(
        &mut self,
        index: usize,
        text: &str,
        cx: &mut Context<Self>,
    ) -> Entity<TextViewState> {
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
        let message = self.chat.messages()[index].clone();
        if let Some(tool) = message.tool_call() {
            return self.render_tool_message(index, tool, p, window, cx);
        }
        if message.role() == Role::User {
            return self.render_user_message(index, &message, p);
        }
        if message.role() == Role::Thought {
            return self.render_thought_message(index, &message, p, cx);
        }
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
                    .rounded(px(10.))
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
                            header.child(Icon::new(IconName::Activity).size(px(14.)))
                        })
                        .child(message.title().to_owned()),
                )
            })
            .child(
                TextView::new(&self.markdown_state(index, message.text(), cx))
                    .style(markdown_style())
                    .selectable(true)
                    .scrollable(false)
                    .w_full()
                    .text_sm()
                    .line_height(rems(1.375))
                    .into_any_element(),
            )
            .when_some(self.images.get(&index), |body, images| {
                body.child(div().id(("images", index)).w_full().flex().flex_col().gap_1().children(
                    images.iter().map(|image| {
                        img(image.clone())
                            .w_full()
                            .h(px(200.))
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
