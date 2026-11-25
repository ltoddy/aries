use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::settings::palette::Palette;
use crate::workspace::sidebar::project::render_project;
use crate::workspace::sidebar::toolbar::render_toolbar;

pub fn render_sessions(
    owner: &SessionView,
    palette: Palette,
    cx: &mut Context<SessionView>,
) -> impl IntoElement {
    let list = &owner.sidebar().list();
    let groups = owner.sidebar().motion().projects();
    let empty = groups.is_empty();
    div()
        .flex_1()
        .min_h_0()
        .flex()
        .flex_col()
        .child(div().px_2().child(render_toolbar(owner, palette, cx)))
        .when(owner.management().is_pending(), |list| {
            list.child(
                div().px_4().py_1().text_xs().text_color(palette.muted()).child("正在更新会话…"),
            )
        })
        .child(
            div()
                .id("session-list")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(owner.sidebar().scroll())
                .px_2()
                .pb_2()
                .children(
                    groups
                        .into_iter()
                        .map(|(cwd, sessions)| render_project(owner, cwd, &sessions, palette, cx)),
                )
                .when(list.is_loading(), |list| {
                    list.child(
                        div().p_2().text_xs().text_color(palette.muted()).child("正在加载会话…"),
                    )
                })
                .when(list.is_unsupported(), |list| {
                    list.child(
                        div()
                            .p_2()
                            .text_xs()
                            .text_color(palette.muted())
                            .child("Agent 不支持会话列表"),
                    )
                })
                .when(
                    list.is_supported() && !list.is_loading() && list.error().is_none() && empty,
                    |list| {
                        list.child(div().p_2().text_xs().text_color(palette.muted()).child(
                            if owner.sidebar().list().show_archived() {
                                "暂无已归档会话"
                            } else {
                                "暂无历史会话"
                            },
                        ))
                    },
                ),
        )
}
