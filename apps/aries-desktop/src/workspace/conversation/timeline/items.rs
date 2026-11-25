use crate::workspace::conversation::timeline::*;

impl Timeline {
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
        palette: Palette,
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
                ElementId::NamedChild(
                    ElementId::from(("thought-motion", cx.entity_id())).into(),
                    match session_id {
                        Some(id) => format!("session:{id}").into(),
                        None => "no-session".into(),
                    },
                )
                .into(),
                index.to_string().into(),
            );
            let markdown = expanded.then(|| self.markdown_state(index, conversation, cx));
            return ThoughtMessage::from_message(
                index,
                palette,
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
            .with_session(cx.weak_entity())
            .into_any_element();
        }
        let message = &conversation.messages()[index];
        if let Some(subagent) = message.subagent() {
            let collapsed = !self.expanded_subagents.contains(subagent.session_id());
            let id = subagent.session_id().clone();
            return SubagentMessage::from_subagent(
                subagent,
                palette,
                collapsed,
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.with_timeline(|timeline, _, _| timeline.toggle_subagent(&id));
                    cx.notify();
                }),
            )
            .with_session(cx.weak_entity())
            .into_any_element();
        }
        if let Some(tool) = message.tool_call() {
            return ToolMessage::from_tool(
                index,
                tool,
                palette,
                cx.entity_id(),
                session_id,
                self.tool_diffs.render(index, tool, palette, cx.weak_entity()),
                cx.listener(move |this, _: &ClickEvent, window, cx| {
                    this.open_tool_details(index, window, cx);
                }),
            )
            .into_any_element();
        }
        if message.role() == Role::User {
            return UserMessage::from_message(
                index,
                message,
                &self.images,
                palette,
                cx.weak_entity(),
            )
            .into_any_element();
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
            .when(card, |body| body.px_2().py_1().rounded(cx.theme().radius).bg(palette.bubble()))
            .when(card || !message.title().is_empty(), |body| {
                body.child(
                    div()
                        .id(("message-header", index))
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .text_color(palette.muted())
                        .when(card, |header| {
                            header.child(Icon::new(IconName::Activity).size(rems(0.875)))
                        })
                        .when_some(
                            compaction.zip(compaction_status),
                            |header, (state, (_, icon))| {
                                let color = if state.status == CompactionStatus::Failed {
                                    palette.danger()
                                } else {
                                    palette.muted()
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
                                            ElementId::NamedChild(
                                                ElementId::from((
                                                    "compaction-status-motion",
                                                    cx.entity_id(),
                                                ))
                                                .into(),
                                                match session_id {
                                                    Some(id) => format!("session:{id}").into(),
                                                    None => "no-session".into(),
                                                },
                                            )
                                            .into(),
                                            state.compaction_id.to_string().into(),
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
                        .map(typography::style_markdown)
                        .selectable(true)
                        .scrollable(false)
                        .on_link_click(link_handler(cx.weak_entity()))
                        .w_full()
                        .code_block_actions(move |code_block, _window, _cx| {
                            let code = code_block.code().to_string();
                            div()
                                .flex()
                                .items_center()
                                .gap_1()
                                .when_some(code_block.lang(), |el, lang| {
                                    el.child(
                                        div()
                                            .text_xs()
                                            .text_color(palette.muted())
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
                                                .text_color(palette.muted()),
                                        )
                                })
                                .into_any_element()
                        })
                        .into_any_element()
                }),
            )
            .children(message.updates().iter().enumerate().filter_map(
                |(attachment_index, update)| {
                    let content = match &update.update {
                        SessionUpdate::AgentMessageChunk(chunk)
                        | SessionUpdate::AgentThoughtChunk(chunk) => &chunk.content,
                        _ => return None,
                    };
                    resource_uri(content)?;
                    Some(
                        Attachment::from_content(index, attachment_index, content, None, palette)
                            .with_session(cx.weak_entity())
                            .into_any_element(),
                    )
                },
            ))
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
