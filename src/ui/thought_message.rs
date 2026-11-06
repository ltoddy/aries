use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::session::chat::Message;
use crate::ui::ChatView;
use crate::ui::palette::Palette;

impl ChatView {
    pub fn render_thought_message(
        &mut self,
        index: usize,
        message: &Message,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let expanded = if self.chat.is_streaming(index) {
            !self.collapsed_streaming_thoughts.contains(&index)
        } else {
            self.expanded_thoughts.contains(&index)
        };
        div()
            .id(("message", index))
            .aria_label(message.title().to_owned())
            .aria_value(message.text().to_owned())
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                Button::new(("toggle-thought", index))
                    .ghost()
                    .xsmall()
                    .w_full()
                    .h_6()
                    .px_0()
                    .text_color(p.muted())
                    .accessibility_label(if expanded {
                        "收起思考过程"
                    } else {
                        "展开思考过程"
                    })
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_left()
                            .child(
                                Icon::new(if expanded {
                                    IconName::ChevronDown
                                } else {
                                    IconName::ChevronRight
                                })
                                .size(px(14.))
                                .flex_shrink_0(),
                            )
                            .child(Icon::new(IconName::Brain).size(px(14.)).flex_shrink_0())
                            .child(message.title().to_owned()),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let toggled = if this.chat.is_streaming(index) {
                            &mut this.collapsed_streaming_thoughts
                        } else {
                            &mut this.expanded_thoughts
                        };
                        if !toggled.remove(&index) {
                            toggled.insert(index);
                        }
                        cx.notify();
                    })),
            )
            .when(expanded, |body| {
                body.child(
                    div().w_full().min_w_0().child(
                        TextView::new(&self.markdown_state(index, message.text(), cx))
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
                            .text_xs()
                            .line_height(rems(1.125))
                            .text_color(p.muted()),
                    ),
                )
            })
            .into_any_element()
    }
}
