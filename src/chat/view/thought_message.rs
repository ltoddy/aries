use std::f32::consts::FRAC_PI_2;

use gpui_kit::assets::IconName;
use gpui_kit::base::motion::{self, Presence, Transition};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;

impl ChatView {
    pub fn render_thought_message(
        &mut self,
        index: usize,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let expanded = if self.chat.is_streaming(index) {
            !self.collapsed_streaming_thoughts.contains(&index)
        } else {
            self.expanded_thoughts.contains(&index)
        };
        let id = ElementId::NamedChild(
            ElementId::from(("thought-motion", cx.entity_id())).into(),
            format!("{:?}-{index}", self.session_id).into(),
        );
        let markdown = expanded.then(|| self.markdown_state(index, cx));
        let message = &self.chat.messages()[index];
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
                            .child(ThoughtChevron { id: id.clone(), expanded })
                            .child(Icon::new(IconName::Brain).size(rems(0.875)).flex_shrink_0())
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
            .when_some(markdown, |body, markdown| {
                body.child(ThoughtBody {
                    id,
                    child: div()
                        .w_full()
                        .min_w_0()
                        .child(
                            TextView::new(&markdown)
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
                                                    cx.write_to_clipboard(
                                                        ClipboardItem::new_string(code.clone()),
                                                    );
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
                                .line_height(rems(1.375))
                                .text_color(p.muted()),
                        )
                        .into_any_element(),
                })
            })
            .into_any_element()
    }
}

#[derive(IntoElement)]
struct ThoughtChevron {
    id: ElementId,
    expanded: bool,
}

impl RenderOnce for ThoughtChevron {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion = cx.theme().motion_tokens();
        let rotation = motion::transition(
            (self.id, "chevron"),
            if self.expanded { FRAC_PI_2 } else { 0. },
            Transition::new(motion.duration_fast).easing(motion.easing_move.clone()),
            window,
            cx,
        );
        Icon::new(IconName::ChevronRight)
            .rotate(radians(rotation))
            .size(rems(0.875))
            .flex_shrink_0()
    }
}

#[derive(IntoElement)]
struct ThoughtBody {
    id: ElementId,
    child: AnyElement,
}

impl RenderOnce for ThoughtBody {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion = cx.theme().motion_tokens();
        let presence = Presence::new((self.id, "body"), true)
            .transition(Transition::new(motion.duration_fast).easing(motion.easing_enter.clone()))
            .sample(window, cx);
        div().w_full().min_w_0().opacity(presence.progress).child(self.child)
    }
}
