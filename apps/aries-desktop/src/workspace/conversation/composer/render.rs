use crate::workspace::conversation::composer::*;

use crate::workspace::settings::SendKey;
use crate::workspace::settings::palette::Palette;
use crate::workspace::settings::typography;

impl Composer {
    fn render_attachments(&self, p: Palette, cx: &mut Context<SessionView>) -> AnyElement {
        div()
            .id("draft-attachments")
            .flex()
            .flex_wrap()
            .gap_2()
            .children(self.attachments.iter().map(|attachment| {
                let id = attachment.id();
                let name = attachment.name().to_owned();
                let icon = if attachment.mime_type().starts_with("image/") {
                    IconName::Image
                } else if attachment.mime_type().starts_with("audio/") {
                    IconName::FileMusic
                } else {
                    IconName::FileText
                };
                div()
                    .id(("draft-attachment", id))
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_2()
                    .w_64()
                    .flex_shrink_0()
                    .max_w_full()
                    .min_w_0()
                    .rounded(cx.theme().radius)
                    .bg(p.bubble())
                    .child(match attachment.image() {
                        Some(image) => Button::new(("attachment-thumbnail", id))
                            .ghost()
                            .size_12()
                            .p_1()
                            .flex_shrink_0()
                            .accessibility_label(format!("预览图片：{}", attachment.name()))
                            .child(
                                img(image)
                                    .size_10()
                                    .rounded(cx.theme().radius)
                                    .object_fit(ObjectFit::Contain)
                                    .with_fallback(move || Icon::new(icon).into_any_element()),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_attachment_preview(id, window, cx);
                            }))
                            .into_any_element(),
                        None => Icon::new(icon).size_6().into_any_element(),
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .id(("attachment-name", id))
                                    .truncate()
                                    .text_sm()
                                    .child(name.clone())
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(name.clone()).build(window, cx)
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_xs()
                                    .text_color(p.muted())
                                    .child(format!("{:.1} KiB", attachment.size() as f64 / 1024.))
                                    .when(attachment.image().is_some(), |row| {
                                        row.child(
                                            Button::new(("preview-attachment", id))
                                                .ghost()
                                                .xsmall()
                                                .label("预览…")
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.open_attachment_preview(
                                                            id, window, cx,
                                                        );
                                                    },
                                                )),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        Button::new(("remove-attachment", id))
                            .ghost()
                            .small()
                            .icon(IconName::X)
                            .accessibility_label(format!("移除附件：{}", attachment.name()))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.with_composer(|composer| {
                                    composer.attachments.retain(|attachment| attachment.id() != id)
                                });
                                this.input().update(cx, |input, cx| input.focus(window, cx));
                                cx.notify();
                            })),
                    )
            }))
            .when(self.attachment_task.is_some(), |row| {
                row.child(div().text_sm().text_color(p.muted()).child("正在添加附件…"))
            })
            .into_any_element()
    }

    pub fn render(
        &self,
        session: &SessionView,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) -> impl IntoElement {
        let rem = window.rem_size();
        let input_height =
            rem * (6. * 1.25 + 1. + 0.25 + 2. * 1.5 + 0.5) + Size::Medium.input_py() * 2. + px(2.);
        let remaining_height =
            (window.viewport_size().height - rem * (2.25 + 0.375 + 1.25 + 1.) - input_height)
                .max(Pixels::ZERO);
        let footer_height = (rem * 3.).min(remaining_height * 0.4);
        let auxiliary_height = remaining_height - footer_height;
        let can_send = session.can_send(cx);
        let send_key = session.send_key(cx);
        div().flex_shrink_0().min_w_0().pl_8().pr_3().pt_1().pb_2().child(
            div()
                .w_full()
                .max_w(rems(60.))
                .mx_auto()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .id("composer-auxiliary")
                        .min_h_0()
                        .max_h(auxiliary_height)
                        .overflow_y_scrollbar()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .when_some(session.execution().permission(), |composer, permission| {
                            composer.child(PermissionPrompt::new(
                                ("permission-motion", cx.entity_id()).into(),
                                div()
                                    .p_3()
                                    .rounded(cx.theme().radius_lg)
                                    .bg(p.bubble())
                                    .child(
                                        div()
                                            .mb_2()
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(permission.title().to_owned()),
                                    )
                                    .child(div().flex().flex_wrap().gap_2().children(
                                        permission.options().iter().enumerate().map(
                                            |(index, option)| {
                                                let id = option.option_id.clone();
                                                Button::new(("permission", index))
                                                    .label(option.name.clone())
                                                    .small()
                                                    .on_click(cx.listener(move |this, _, _, cx| {
                                                        this.choose_permission(id.clone(), cx)
                                                    }))
                                            },
                                        ),
                                    ))
                                    .into_any_element(),
                            ))
                        })
                        .child(session.render_execution_context(p, cx))
                        .when_some(session.execution().prompt_queue(), |composer, queue| {
                            composer.child(queue.render(session, p, cx))
                        })
                        .child(self.render_attachments(p, cx)),
                )
                .child(
                    div()
                        .relative()
                        .flex_shrink_0()
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
                            if !this.input().read(cx).focus_handle(cx).is_focused(window)
                                || this.input().update(cx, |input, cx| {
                                    input.marked_text_range(window, cx).is_some()
                                })
                            {
                                return;
                            }
                            if !action.shift && !action.secondary {
                                this.command_menu_action(CommandMenuAction::Complete, window, cx);
                                if window.default_prevented() {
                                    return;
                                }
                            }
                            if !action.shift
                                && action.secondary == (this.send_key(cx) == SendKey::CommandEnter)
                            {
                                window.prevent_default();
                                cx.stop_propagation();
                                this.send(window, cx);
                            }
                        }))
                        .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                            this.command_menu_action(CommandMenuAction::Complete, window, cx)
                        }))
                        .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                            this.command_menu_action(CommandMenuAction::Dismiss, window, cx)
                        }))
                        .children(self.render_commands(
                            session.execution().conversation().available_commands(),
                            session.commands_available(),
                            session.execution().session_id().is_some(),
                            p,
                            cx,
                        ))
                        .child(ComposerInput::new(self, session, p, cx.weak_entity(), can_send)),
                )
                .child(
                    div()
                        .id("composer-footer")
                        .min_h_0()
                        .max_h(footer_height)
                        .overflow_y_scrollbar()
                        .flex()
                        .items_center()
                        .flex_wrap()
                        .gap_2()
                        .min_w_0()
                        .child(div().flex_1().min_w_0().when(
                            !session.execution().conversation().config_options().is_empty()
                                || session.configuration().error().is_some(),
                            |footer| footer.child(session.render_session_config(p, cx)),
                        ))
                        .child(
                            div()
                                .min_w_0()
                                .ml_auto()
                                .text_xs()
                                .text_color(p.muted())
                                .whitespace_normal()
                                .text_right()
                                .child(format!(
                                    "{} {} · {} 换行",
                                    send_key.send_label(),
                                    if session.execution().conversation().is_busy() {
                                        "加入队列"
                                    } else if session.execution().prompt_queue().is_some() {
                                        "逐条发送"
                                    } else {
                                        "发送"
                                    },
                                    send_key.newline_label(),
                                )),
                        ),
                ),
        )
    }
}

#[derive(IntoElement)]
struct PermissionPrompt {
    id: ElementId,
    child: AnyElement,
}

impl PermissionPrompt {
    fn new(id: ElementId, child: AnyElement) -> Self {
        Self { id, child }
    }
}

impl RenderOnce for PermissionPrompt {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion = cx.theme().motion_tokens();
        let distance = motion.distance_short;
        let transition =
            Transition::new(motion.duration_normal).easing(motion.easing_enter.clone());
        let presence = Presence::new(self.id, true).transition(transition).sample(window, cx);
        div().w_full().child(
            div()
                .relative()
                .top(distance * (1. - presence.progress))
                .opacity(presence.progress)
                .child(self.child),
        )
    }
}

#[derive(IntoElement)]
struct ComposerInput {
    input: Entity<TextareaState>,
    busy: bool,
    stopping: bool,
    can_send: bool,
    palette: Palette,
    attachments_available: bool,
    view: WeakEntity<SessionView>,
    has_queue: bool,
}
impl ComposerInput {
    fn new(
        composer: &Composer,
        session: &SessionView,
        palette: Palette,
        view: WeakEntity<SessionView>,
        can_send: bool,
    ) -> Self {
        Self {
            input: composer.input.clone(),
            busy: session.execution().conversation().is_busy(),
            stopping: session.execution().is_stopping(),
            can_send,
            palette,
            attachments_available: session.attachments_available()
                && composer.attachment_task.is_none(),
            view,
            has_queue: session.execution().prompt_queue().is_some(),
        }
    }
}

impl RenderOnce for ComposerInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            input,
            busy,
            stopping,
            can_send,
            palette: p,
            attachments_available,
            view,
            has_queue,
        } = self;
        let input_focused = input.read(cx).focus_handle(cx).is_focused(window);
        let picker = view.clone();
        let drop_view = view.clone();
        let stop_view = view.clone();
        let send_view = view.clone();
        div()
            .id("multimodal-composer")
            .on_drop(move |paths: &ExternalPaths, window, cx| {
                let _ = drop_view
                    .update(cx, |this, cx| this.attach_paths(paths.0.iter().cloned(), window, cx));
            })
            .when(attachments_available, |composer| {
                composer.drag_over::<ExternalPaths>(|style, _, _, cx| {
                    style.border_color(cx.theme().primary)
                })
            })
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded(cx.theme().radius_2xl())
            .border_1()
            .border_color(p.border())
            .when(input_focused, |surface| surface.border_color(cx.theme().ring))
            .bg(p.input())
            .child(
                div().id("composer-input").w_full().min_w_0().child(
                    Textarea::new(&input)
                        .appearance(false)
                        .bordered(false)
                        .aria_label("消息")
                        .rounded(cx.theme().radius)
                        .text_size(typography::body_size())
                        .w_full()
                        .on_paste(move |item, window, cx| {
                            view.update(cx, |this, cx| this.paste_attachments(item, window, cx))
                                .unwrap_or(false)
                        }),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .min_w_0()
                    .when(busy, |composer| {
                        composer.child(
                            Button::new("stop")
                                .ghost()
                                .small()
                                .rounded(cx.theme().radius)
                                .text_color(p.muted())
                                .icon(IconName::Square)
                                .label(if stopping { "停止中" } else { "停止" })
                                .accessibility_label("停止回复")
                                .disabled(stopping)
                                .on_click(move |_, window, cx| {
                                    let _ = stop_view.update(cx, |this, cx| this.stop(window, cx));
                                }),
                        )
                    })
                    .child(
                        Button::new("add-attachments")
                            .ghost()
                            .icon(IconName::Paperclip)
                            .accessibility_label("添加附件…")
                            .tooltip("添加图片、音频、PDF 或文本…")
                            .small()
                            .rounded(cx.theme().radius)
                            .text_color(p.muted())
                            .disabled(!attachments_available)
                            .on_click(move |_, window, cx| {
                                let _ = picker
                                    .update(cx, |this, cx| this.choose_attachments(window, cx));
                            }),
                    )
                    .child(
                        Button::new("send")
                            .primary()
                            .small()
                            .rounded(cx.theme().radius)
                            .icon(IconName::Send)
                            .when(!busy && !has_queue, |button| button.size_8())
                            .when(busy || has_queue, |button| {
                                button.label(if busy { "加入队列" } else { "逐条发送" })
                            })
                            .accessibility_label(if busy {
                                "加入待发送队列"
                            } else if has_queue {
                                "将消息加入队列并逐条发送"
                            } else {
                                "发送消息"
                            })
                            .disabled(!can_send)
                            .on_click(move |_, window, cx| {
                                let _ = send_view.update(cx, |this, cx| this.send(window, cx));
                            }),
                    ),
            )
    }
}
