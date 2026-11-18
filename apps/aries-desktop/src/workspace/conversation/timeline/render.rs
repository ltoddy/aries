use crate::workspace::conversation::timeline::*;

use gpui_kit::IntoElement;

impl Timeline {
    pub fn render_timeline(
        &mut self,
        conversation: &Conversation,
        loading: bool,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> AnyElement {
        if conversation.messages().is_empty() && conversation.error().is_none() && !loading {
            return EmptyTimeline { palette: p }.into_any_element();
        }
        self.sync_timeline_items(conversation);
        let messages = conversation.messages().len();
        let view = cx.weak_entity();
        let timeline = self.list.clone();
        div()
            .id("timeline")
            .flex_1()
            .min_h_0()
            .pl_8()
            .pr_3()
            .py_1()
            .relative()
            .child(
                div().w_full().h_full().child(
                    list(timeline, move |index, _, cx| {
                        view.update(cx, |this, cx| {
                            this.with_timeline(|timeline, conversation, session_id| {
                                let gap = match conversation.messages().get(index).map(|m| m.role())
                                {
                                    Some(Role::User) => rems(0.75),
                                    _ => rems(0.375),
                                };
                                div()
                                    .w_full()
                                    .pb(gap)
                                    .child(div().max_w(rems(60.)).mx_auto().child(
                                        if index < messages {
                                            timeline.timeline_item(
                                                index,
                                                conversation,
                                                session_id,
                                                p,
                                                cx,
                                            )
                                        } else {
                                            div()
                                                .id("chat-error")
                                                .rounded(cx.theme().radius_lg)
                                                .p_3()
                                                .bg(p.bubble())
                                                .text_color(p.danger())
                                                .child(SelectableText::new(
                                                    "chat-error-text",
                                                    conversation
                                                        .error()
                                                        .unwrap_or_default()
                                                        .to_owned(),
                                                ))
                                                .into_any_element()
                                        },
                                    ))
                                    .into_any_element()
                            })
                        })
                        .unwrap_or_else(|_| div().into_any_element())
                    })
                    .w_full()
                    .h_full(),
                ),
            )
            .child(self.render_user_anchors(conversation, p, cx))
            .child(self.render_scroll_controls(conversation, p, cx))
            .when(!self.list.is_following_tail(), |el| {
                el.child(
                    div().absolute().bottom_4().left_0().right_0().flex().justify_center().child(
                        Button::new("scroll-to-bottom")
                            .ghost()
                            .icon(IconName::ArrowDown)
                            .size_8()
                            .rounded(cx.theme().radius_full())
                            .bg(p.input())
                            .text_color(p.muted())
                            .accessibility_label("回到最新消息并恢复自动跟随")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.with_timeline(|timeline, _, _| {
                                    timeline.list().set_follow_mode(FollowMode::Tail);
                                });
                                cx.notify();
                            })),
                    ),
                )
            })
            .into_any_element()
    }

    fn render_scroll_controls(
        &self,
        conversation: &Conversation,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> AnyElement {
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
                            .ghost()
                            .small()
                            .bg(p.input())
                            .icon(IconName::ArrowUpToLine)
                            .accessibility_label("回到顶部")
                            .tooltip("回到顶部")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.with_timeline(|timeline, _, _| {
                                    timeline.list().scroll_to(ListOffset::default());
                                });
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("previous-user-message")
                            .ghost()
                            .small()
                            .bg(p.input())
                            .icon(IconName::ArrowUp)
                            .accessibility_label("上一条用户消息")
                            .tooltip_with_action("上一条用户消息", &PreviousUserMessage, None)
                            .disabled(self.user_message_target(conversation, false).is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.jump_user_message(false, cx);
                            })),
                    )
                    .child(
                        Button::new("next-user-message")
                            .ghost()
                            .small()
                            .bg(p.input())
                            .icon(IconName::ArrowDown)
                            .accessibility_label("下一条用户消息")
                            .tooltip_with_action("下一条用户消息", &NextUserMessage, None)
                            .disabled(self.user_message_target(conversation, true).is_none())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.jump_user_message(true, cx);
                            })),
                    ),
            )
            .into_any_element()
    }

    fn render_user_anchors(
        &self,
        conversation: &Conversation,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> AnyElement {
        let timeline = self.list.clone();
        let view = cx.weak_entity();
        let anchors = conversation
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
}

#[derive(IntoElement)]
struct EmptyTimeline {
    palette: Palette,
}
impl RenderOnce for EmptyTimeline {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let p = self.palette;
        div().flex_1().min_h_0().px_6().flex().items_center().justify_center().child(
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
                        .rounded(cx.theme().radius_lg)
                        .bg(p.bubble())
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(Icon::new(IconName::Bot).size(rems(1.625))),
                )
                .child(
                    div()
                        .mt_1()
                        .text_lg()
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
    }
}
