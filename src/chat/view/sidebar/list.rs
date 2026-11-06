use std::collections::BTreeSet;
use std::path::Path;

use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;

impl ChatView {
    pub fn render_sessions(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let list = &self.session_list;
        let current_listed = self.session_id.as_ref().is_some_and(|id| list.session(id).is_some());
        let mut groups: BTreeSet<&Path> = list.projects().collect();
        if !current_listed {
            groups.insert(&self.cwd);
        }
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
                    .children(groups.into_iter().map(|cwd| {
                        self.render_project(cwd, !current_listed && cwd == self.cwd, p, cx)
                    }))
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
                            && list.entries().is_empty(),
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
