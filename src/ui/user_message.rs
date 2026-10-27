use gpui_kit::component::{Icon, IconName, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::session::chat::Message;
use crate::ui::ChatView;
use crate::ui::palette::Palette;

impl ChatView {
    pub fn render_user_message(
        &mut self,
        index: usize,
        message: &Message,
        p: Palette,
    ) -> AnyElement {
        div()
            .id(("message", index))
            .aria_label("用户消息")
            .aria_value(message.text().to_owned())
            .group("user-message")
            .w_full()
            .flex()
            .items_end()
            .justify_end()
            .gap_1()
            .when(!message.text().is_empty(), |row| {
                let text = message.text().to_owned();
                row.child(
                    div()
                        .id(("copy-message", index))
                        .w(px(24.))
                        .h_6()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .aria_label("复制用户消息")
                        .opacity(0.45)
                        .group_hover("user-message", |this| this.opacity(1.))
                        .on_click(move |_, _, cx| {
                            cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                        })
                        .child(Icon::new(IconName::Copy).xsmall().text_color(p.muted())),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .max_w(relative(0.72))
                    .px_2()
                    .py_1()
                    .rounded(px(16.))
                    .bg(p.bubble())
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(message.attachments().map(|(attachment_index, content)| {
                        self.render_attachment(index, attachment_index, content, p)
                    }))
                    .when(!message.text().is_empty(), |bubble| {
                        bubble.child(
                            div()
                                .whitespace_normal()
                                .line_height(rems(1.5))
                                .child(message.text().to_owned()),
                        )
                    }),
            )
            .into_any_element()
    }
}
