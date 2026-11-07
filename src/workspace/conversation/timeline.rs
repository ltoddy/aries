use std::collections::{HashMap, HashSet};
use std::time::Duration;

use agent_client_protocol::schema::MaybeUndefined;
use agent_client_protocol::v1::schema::{CompactionStatus, SessionId};
use gpui_kit::assets::IconName;
use gpui_kit::base::SelectableText;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::{TextView, TextViewState, TextViewStyle};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::conversation::media::MessageImages;
use crate::workspace::conversation::model::display::UpdateDisplay;
use crate::workspace::conversation::model::subagent::SubagentState;
use crate::workspace::conversation::model::{Conversation, Role};
use crate::workspace::conversation::subagent_message::SubagentMessage;
use crate::workspace::conversation::thought_message::ThoughtMessage;
use crate::workspace::conversation::tool_content::ToolDiffs;
use crate::workspace::conversation::tool_message::{MessageStatusIcon, ToolMessage};
use crate::workspace::conversation::user_message::UserMessage;
use crate::workspace::settings::palette::Palette;
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

pub struct Timeline {
    list: ListState,
    markdown: HashMap<usize, Entity<TextViewState>>,
    expanded_thoughts: HashSet<usize>,
    collapsed_streaming_thoughts: HashSet<usize>,
    collapsed_subagents: HashSet<SessionId>,
    idle_subagents: HashSet<SessionId>,
    images: MessageImages,
    tool_diffs: ToolDiffs,
}

impl Timeline {
    pub fn new(cx: &mut Context<SessionView>) -> Self {
        let list = ListState::new(0, ListAlignment::Bottom, px(600.));
        list.set_follow_mode(FollowMode::Tail);
        let view = cx.weak_entity();
        list.set_scroll_handler(move |_, _, cx| {
            let _ = view.update(cx, |_, cx| cx.notify());
        });
        Self {
            list,
            markdown: HashMap::new(),
            expanded_thoughts: HashSet::new(),
            collapsed_streaming_thoughts: HashSet::new(),
            collapsed_subagents: HashSet::new(),
            idle_subagents: HashSet::new(),
            images: MessageImages::new(),
            tool_diffs: ToolDiffs::new(),
        }
    }
    pub fn list(&self) -> &ListState {
        &self.list
    }
    pub fn reset(&mut self) {
        self.markdown.clear();
        self.expanded_thoughts.clear();
        self.collapsed_streaming_thoughts.clear();
        self.collapsed_subagents.clear();
        self.idle_subagents.clear();
        self.tool_diffs.clear();
        self.images.rebuild_images();
        self.list.reset(0);
        self.list.set_follow_mode(FollowMode::Tail);
    }
    pub fn cache_message_images(&mut self, index: usize, conversation: &Conversation) {
        self.images.cache_message_images(index, &conversation.messages()[index]);
    }
    pub fn rebuild_images(&mut self) {
        self.images.rebuild_images();
    }
    pub fn cache_tool_diffs(&mut self, index: usize, conversation: &Conversation) {
        self.tool_diffs.cache_tool_diffs(index, conversation);
    }
    pub fn rebuild_tool_diffs(&mut self, conversation: &Conversation) {
        self.tool_diffs.rebuild_tool_diffs(conversation);
    }
    pub fn toggle_thought(&mut self, index: usize, conversation: &Conversation) {
        let toggled = if conversation.is_streaming(index) {
            &mut self.collapsed_streaming_thoughts
        } else {
            &mut self.expanded_thoughts
        };
        if !toggled.remove(&index) {
            toggled.insert(index);
        }
    }
    pub fn sync_subagent(&mut self, subagent: &SubagentState) {
        let id = subagent.session_id();
        if subagent.is_idle() {
            if self.idle_subagents.insert(id.clone()) {
                self.collapsed_subagents.insert(id.clone());
            }
        } else {
            self.idle_subagents.remove(id);
        }
    }

    pub fn toggle_subagent(&mut self, id: &SessionId) {
        if !self.collapsed_subagents.remove(id) {
            self.collapsed_subagents.insert(id.clone());
        }
    }

    fn user_message_target(&self, conversation: &Conversation, next: bool) -> Option<usize> {
        let offset = self.list.logical_scroll_top();
        let anchors = conversation
            .messages()
            .iter()
            .enumerate()
            .filter_map(|(index, message)| (message.role() == Role::User).then_some(index));
        user_message_target(anchors, offset.item_ix, offset.offset_in_item > px(0.), next)
    }

    pub fn jump_user_message(
        &mut self,
        conversation: &Conversation,
        next: bool,
        cx: &mut Context<SessionView>,
    ) {
        if let Some(item_ix) = self.user_message_target(conversation, next) {
            self.list.scroll_to(ListOffset { item_ix, offset_in_item: px(0.) });
            cx.notify();
        }
    }

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
            .pr_4()
            .py_2()
            .relative()
            .child(
                div().w_full().h_full().child(
                    list(timeline, move |index, _, cx| {
                        view.update(cx, |this, cx| {
                            this.with_timeline(|timeline, conversation, session_id| {
                                let gap = match conversation.messages().get(index).map(|m| m.role())
                                {
                                    Some(Role::User) => rems(0.75),
                                    _ => rems(0.25),
                                };
                                div()
                                    .w_full()
                                    .pb(gap)
                                    .child(div().max_w(rems(55.)).mx_auto().child(
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
                                                .rounded(rems(0.75))
                                                .p_2()
                                                .border_1()
                                                .border_color(p.border())
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
                            .icon(IconName::ArrowDown)
                            .size(rems(2.5))
                            .rounded_full()
                            .bg(p.bubble())
                            .opacity(0.7)
                            .shadow_md()
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
                            .outline()
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
                            .outline()
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
                            .outline()
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

    fn sync_timeline_items(&self, conversation: &Conversation) {
        let count = conversation.messages().len() + usize::from(conversation.error().is_some());
        let current = self.list.item_count();
        if current < count {
            self.list.splice(current..current, count - current);
        } else if current > count {
            self.list.splice(count..current, 0);
        }
    }

    pub fn markdown_state(
        &mut self,
        index: usize,
        conversation: &Conversation,
        cx: &mut Context<SessionView>,
    ) -> Entity<TextViewState> {
        let message = &conversation.messages()[index];
        let compaction_body = message.compaction().map(|state| {
            let mut error = state.clone();
            error.summary = MaybeUndefined::Undefined;
            UpdateDisplay::from(&error).into_parts().1
        });
        let text = compaction_body.as_deref().unwrap_or_else(|| message.text());
        let value = if text.is_empty() { "（空内容）" } else { text };
        let state = self
            .markdown
            .entry(index)
            .or_insert_with(|| cx.new(|cx| TextViewState::markdown(value, cx)))
            .clone();
        state.update(cx, |state, cx| state.set_text(value, cx));
        state
    }

    pub fn timeline_item(
        &mut self,
        index: usize,
        conversation: &Conversation,
        session_id: Option<&SessionId>,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> AnyElement {
        self.images.cache_visible_images(index, &conversation.messages()[index]);
        if conversation.messages()[index].role() == Role::Thought {
            let expanded = if conversation.is_streaming(index) {
                !self.collapsed_streaming_thoughts.contains(&index)
            } else {
                self.expanded_thoughts.contains(&index)
            };
            let id = ElementId::NamedChild(
                ElementId::from(("thought-motion", cx.entity_id())).into(),
                format!("{session_id:?}-{index}").into(),
            );
            let markdown = expanded.then(|| self.markdown_state(index, conversation, cx));
            return ThoughtMessage::from_message(
                index,
                p,
                &conversation.messages()[index],
                id,
                expanded,
                markdown,
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.with_timeline(|timeline, conversation, _| {
                        timeline.toggle_thought(index, conversation);
                    });
                    cx.notify();
                }),
            )
            .into_any_element();
        }
        let message = &conversation.messages()[index];
        if let Some(subagent) = message.subagent() {
            let collapsed = self.collapsed_subagents.contains(subagent.session_id());
            let id = subagent.session_id().clone();
            return SubagentMessage::from_subagent(
                subagent,
                p,
                collapsed,
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.with_timeline(|timeline, _, _| timeline.toggle_subagent(&id));
                    cx.notify();
                }),
            )
            .into_any_element();
        }
        if let Some(tool) = message.tool_call() {
            return ToolMessage::from_tool(
                index,
                tool,
                p,
                cx.entity_id(),
                session_id,
                self.tool_diffs.render(index, tool, p),
            )
            .into_any_element();
        }
        if message.role() == Role::User {
            return UserMessage::from_message(index, message, &self.images, p).into_any_element();
        }
        let markdown = self.markdown_state(index, conversation, cx);
        let message = &conversation.messages()[index];
        let compaction = message.compaction();
        let compaction_status = compaction.map(|state| match state.status {
            CompactionStatus::InProgress => ("进行中", IconName::Loader),
            CompactionStatus::Completed => ("已完成", IconName::CircleCheck),
            CompactionStatus::Failed => ("失败", IconName::CircleX),
            CompactionStatus::Cancelled => ("已取消", IconName::CirclePause),
            _ => ("未知状态", IconName::CircleQuestionMark),
        });
        let card = message.role() == Role::Event && compaction.is_none();
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
                        .id(("message-header", index))
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(p.muted())
                        .when(card, |header| {
                            header.child(Icon::new(IconName::Activity).size(rems(0.875)))
                        })
                        .when_some(
                            compaction.zip(compaction_status),
                            |header, (state, (_, icon))| {
                                let color = if state.status == CompactionStatus::Failed {
                                    p.danger()
                                } else {
                                    p.muted()
                                };
                                let mut summary = state.clone();
                                summary.error = MaybeUndefined::Undefined;
                                let summary = UpdateDisplay::from(&summary).into_parts().1;
                                header
                                    .h_6()
                                    .when(!summary.is_empty(), |header| {
                                        header.tooltip(move |window, cx| {
                                            gpui_kit::component::tooltip::Tooltip::new(
                                                summary.clone(),
                                            )
                                            .build(window, cx)
                                        })
                                    })
                                    .child(MessageStatusIcon::new(
                                        ElementId::NamedChild(
                                            ElementId::from((
                                                "compaction-status-motion",
                                                cx.entity_id(),
                                            ))
                                            .into(),
                                            format!("{:?}-{}", session_id, state.compaction_id)
                                                .into(),
                                        ),
                                        state.status == CompactionStatus::InProgress,
                                        state.status == CompactionStatus::Completed,
                                        icon,
                                        color,
                                    ))
                                    .child(Icon::new(IconName::Activity).size(rems(0.875)))
                            },
                        )
                        .child(SelectableText::new(
                            ("message-title", index),
                            message.title().to_owned(),
                        )),
                )
            })
            .children(
                (compaction.is_none()
                    || compaction
                        .is_some_and(|state| matches!(&state.error, MaybeUndefined::Value(_))))
                .then(|| {
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
                        .into_any_element()
                }),
            )
            .when_some(self.images.images(index), |body, images| {
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
            .aria_label(match compaction_status {
                Some((status, _)) => format!("上下文压缩 · {status}"),
                None => message.title().to_owned(),
            })
            .aria_value(message.text().to_owned())
            .w_full()
            .flex()
            .child(div().min_w_0().w_full().child(body))
            .into_any_element()
    }
}

#[derive(IntoElement)]
struct EmptyTimeline {
    palette: Palette,
}
impl RenderOnce for EmptyTimeline {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
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
    }
}
