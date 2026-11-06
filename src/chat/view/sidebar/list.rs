use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;

impl ChatView {
    pub fn render_sessions(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let list = &self.session_list;
        let groups = self.sidebar_motion.projects();
        let empty = groups.is_empty();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .px_3()
            .child(self.render_session_toolbar(p, cx))
            .child(self.render_session_actions(p))
            .child(
                div()
                    .id("session-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.session_scroll)
                    .pb_3()
                    .children(groups.into_iter().map(|cwd| self.render_project(&cwd, p, cx)))
                    .when(list.is_loading(), |list| {
                        list.child(
                            div().p_3().text_xs().text_color(p.muted()).child("正在加载会话…"),
                        )
                    })
                    .when(list.is_unsupported(), |list| {
                        list.child(
                            div()
                                .p_3()
                                .text_xs()
                                .text_color(p.muted())
                                .child("Agent 不支持会话列表"),
                        )
                    })
                    .when(
                        list.is_supported()
                            && !list.is_loading()
                            && list.error().is_none()
                            && empty,
                        |list| {
                            list.child(
                                div().p_3().text_xs().text_color(p.muted()).child("暂无历史会话"),
                            )
                        },
                    )
                    .when_some(list.error().map(str::to_owned), |list, error| {
                        list.child(div().p_3().text_xs().text_color(p.danger()).child(error))
                    }),
            )
    }
}
