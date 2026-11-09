use std::f32::consts::FRAC_PI_2;

use gpui_kit::assets::IconName;
use gpui_kit::base::motion::{self, Presence, Transition};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::{TextView, TextViewState};
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::conversation::ClickHandler;
use crate::workspace::conversation::model::Message;
use crate::workspace::settings::palette::Palette;

#[derive(IntoElement)]
pub struct ThoughtMessage {
    index: usize,
    id: ElementId,
    expanded: bool,
    content: ThoughtContent,
    palette: Palette,
    on_toggle: ClickHandler,
}

impl ThoughtMessage {
    pub fn from_message(
        index: usize,
        p: Palette,
        message: &Message,
        id: ElementId,
        expanded: bool,
        markdown: Option<Entity<TextViewState>>,
        on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        let title = message.title().to_owned();
        let text = message.text().to_owned();
        ThoughtMessage::new(
            index,
            id,
            expanded,
            ThoughtContent::new(title, text, markdown),
            p,
            on_toggle,
        )
    }

    fn new(
        index: usize,
        id: ElementId,
        expanded: bool,
        content: ThoughtContent,
        palette: Palette,
        on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self { index, id, expanded, content, palette, on_toggle: Box::new(on_toggle) }
    }
}

struct ThoughtContent {
    title: String,
    text: String,
    markdown: Option<Entity<TextViewState>>,
}

impl ThoughtContent {
    fn new(title: String, text: String, markdown: Option<Entity<TextViewState>>) -> Self {
        Self { title, text, markdown }
    }
}

impl RenderOnce for ThoughtMessage {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self { index, id, expanded, content, palette: p, on_toggle } = self;
        let ThoughtContent { title, text, markdown } = content;
        div()
            .id(("message", index))
            .aria_label(title.clone())
            .aria_value(text)
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
                            .child(ThoughtChevron::new(id.clone(), expanded))
                            .child(Icon::new(IconName::Brain).size(rems(0.875)).flex_shrink_0())
                            .child(title),
                    )
                    .on_click(on_toggle),
            )
            .when_some(markdown, |body, markdown| {
                body.child(ThoughtBody::new(
                    id,
                    div()
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
                ))
            })
            .into_any_element()
    }
}

#[derive(IntoElement)]
struct ThoughtChevron {
    id: ElementId,
    expanded: bool,
}

impl ThoughtChevron {
    fn new(id: ElementId, expanded: bool) -> Self {
        Self { id, expanded }
    }
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

impl ThoughtBody {
    fn new(id: ElementId, child: AnyElement) -> Self {
        Self { id, child }
    }
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
