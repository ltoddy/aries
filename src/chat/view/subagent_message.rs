use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Icon, IconName, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::model::subagent::{SubagentMessage, SubagentState};
use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;
use crate::chat::view::timeline::markdown_style;

impl ChatView {
    pub(super) fn render_subagent_message(
        &self,
        subagent: &SubagentState,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let collapsed = self.collapsed_subagents.contains(subagent.session_id());
        let toggle_label = if collapsed { "展开子 Agent" } else { "折叠子 Agent" };
        let title = subagent.title().unwrap_or("子 Agent").to_owned();
        let description = subagent.description().map(str::to_owned);
        let state = subagent.state().map(str::to_owned);
        div()
            .w_full()
            .rounded(px(10.))
            .border_1()
            .border_color(p.border())
            .bg(p.input())
            .p_2()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                Button::new(format!("subagent-toggle-{}", subagent.session_id()))
                    .ghost()
                    .small()
                    .w_full()
                    .h_auto()
                    .p_1()
                    .accessibility_label(toggle_label)
                    .on_click({
                        let session_id = subagent.session_id().clone();
                        cx.listener(move |this, _, _, cx| {
                            if !this.collapsed_subagents.insert(session_id.clone()) {
                                this.collapsed_subagents.remove(&session_id);
                            }
                            cx.notify();
                        })
                    })
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Icon::new(if collapsed {
                                    IconName::ChevronRight
                                } else {
                                    IconName::ChevronDown
                                })
                                .size(px(14.))
                                .text_color(p.muted()),
                            )
                            .child(Icon::new(IconName::Bot).size(px(14.)).text_color(p.muted()))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .child(
                                        div()
                                            .truncate()
                                            .text_sm()
                                            .text_color(p.foreground())
                                            .child(title),
                                    )
                                    .when_some(description, |this, description| {
                                        this.child(
                                            div()
                                                .truncate()
                                                .text_xs()
                                                .text_color(p.muted())
                                                .child(description),
                                        )
                                    }),
                            )
                            .when_some(state, |this, state| {
                                this.child(
                                    div()
                                        .flex_shrink_0()
                                        .rounded_full()
                                        .border_1()
                                        .border_color(p.border())
                                        .px_2()
                                        .py_1()
                                        .text_size(rems(0.6875))
                                        .text_color(p.muted())
                                        .child(state),
                                )
                            }),
                    ),
            )
            .when(!collapsed && subagent.messages().is_empty(), |this| {
                this.child(div().text_xs().text_color(p.muted()).child("等待子 Agent 输出…"))
            })
            .when(!collapsed, |this| {
                this.children(
                    subagent
                        .messages()
                        .iter()
                        .map(|message| self.render_subagent_entry(message, p)),
                )
            })
            .into_any_element()
    }

    fn render_subagent_entry(&self, message: &SubagentMessage, p: Palette) -> AnyElement {
        div()
            .rounded(px(8.))
            .bg(p.background())
            .px_2()
            .py_1()
            .child(
                TextView::markdown(
                    format!("subagent-message-{}", message.message_id()),
                    message.text().to_owned(),
                )
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
                            let code = code.clone();
                            Button::new("copy-code-block")
                                .ghost()
                                .xsmall()
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                                })
                                .child(
                                    Icon::new(IconName::Copy).xsmall().text_color(cx.theme().muted),
                                )
                        })
                        .into_any_element()
                })
                .text_sm()
                .line_height(rems(1.375))
                .into_any_element(),
            )
            .into_any_element()
    }
}
