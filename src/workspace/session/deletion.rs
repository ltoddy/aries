use gpui_kit::component::Sizable;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::settings::palette::Palette;

pub fn render_session_actions(owner: &SessionView, p: Palette) -> impl IntoElement {
    let actions = &owner.deletion();
    div()
        .flex_shrink_0()
        .when(actions.is_deleting(), |panel| {
            panel.child(div().p_2().text_xs().text_color(p.muted()).child("正在删除会话…"))
        })
        .when_some(actions.error().map(str::to_owned), |panel, error| {
            panel.child(
                div()
                    .id("session-deletion-error")
                    .max_h(px(120.))
                    .overflow_y_scroll()
                    .p_2()
                    .text_xs()
                    .text_color(p.danger())
                    .child(error),
            )
        })
}

pub fn render_session_delete_popup(
    owner: &SessionView,
    anchor: impl AsRef<str>,
    p: Palette,
    cx: &mut Context<SessionView>,
) -> Option<AnyElement> {
    let anchor = anchor.as_ref();
    let pending = owner.deletion().pending_deletion()?;
    if !pending.is_anchor(anchor) {
        return None;
    }
    Some(
        div()
            .absolute()
            .top_0()
            .right(px(28.))
            .p_2()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border())
            .bg(p.background())
            .shadow_lg()
            .occlude()
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new(SharedString::from(format!(
                            "confirm-session-deletion:{anchor}"
                        )))
                        .label("删除")
                        .danger()
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| this.confirm_session_deletion(cx))),
                    )
                    .child(
                        Button::new(SharedString::from(format!(
                            "cancel-session-deletion:{anchor}"
                        )))
                        .label("取消")
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cancel_session_deletion(cx);
                        })),
                    ),
            )
            .into_any_element(),
    )
}
