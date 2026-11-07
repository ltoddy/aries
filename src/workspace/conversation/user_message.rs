use gpui_kit::component::{Icon, IconName, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::conversation::ClickHandler;
use crate::workspace::conversation::attachment::Attachment;
use crate::workspace::conversation::media::MessageImages;
use crate::workspace::conversation::model::Message;
use crate::workspace::settings::palette::Palette;

#[derive(IntoElement)]
pub struct UserMessage {
    index: usize,
    text: String,
    attachments: Vec<AnyElement>,
    palette: Palette,
    on_copy: ClickHandler,
}

impl UserMessage {
    pub fn from_message(
        index: usize,
        message: &Message,
        images: &MessageImages,
        p: Palette,
    ) -> Self {
        let text = message.text().to_owned();
        let copy_text = text.clone();
        let attachments = message
            .attachments()
            .map(|(attachment_index, content)| {
                Attachment::from_content(
                    index,
                    attachment_index,
                    content,
                    images.attachment(index, attachment_index),
                    p,
                )
                .into_any_element()
            })
            .collect();
        UserMessage::new(
            index,
            text,
            attachments,
            p,
            move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                cx.write_to_clipboard(ClipboardItem::new_string(copy_text.clone()));
            },
        )
    }

    fn new(
        index: usize,
        text: String,
        attachments: Vec<AnyElement>,
        palette: Palette,
        on_copy: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self { index, text, attachments, palette, on_copy: Box::new(on_copy) }
    }
}

impl RenderOnce for UserMessage {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self { index, text, attachments, palette: p, on_copy } = self;
        div()
            .id(("message", index))
            .aria_label("用户消息")
            .aria_value(text.clone())
            .group("user-message")
            .w_full()
            .flex()
            .items_end()
            .justify_end()
            .gap_1()
            .when(!text.is_empty(), |row| {
                row.child(
                    div()
                        .id(("copy-message", index))
                        .w(rems(1.5))
                        .h_6()
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .aria_label("复制用户消息")
                        .opacity(0.45)
                        .group_hover("user-message", |this| this.opacity(1.))
                        .on_click(on_copy)
                        .child(Icon::new(IconName::Copy).xsmall().text_color(p.muted())),
                )
            })
            .child(
                div()
                    .min_w_0()
                    .max_w(relative(0.72))
                    .px_2()
                    .py_1()
                    .rounded(rems(1.))
                    .bg(p.bubble())
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(attachments)
                    .when(!text.is_empty(), |bubble| {
                        bubble.child(div().whitespace_normal().line_height(rems(1.5)).child(text))
                    }),
            )
            .into_any_element()
    }
}
