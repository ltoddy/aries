use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::settings::palette::Palette;
use crate::workspace::sidebar::Sidebar;
use crate::workspace::sidebar::project::ProjectView;
use crate::workspace::sidebar::toolbar::SessionToolbar;

pub struct SessionListView<'a> {
    sidebar: &'a Sidebar,
    owner: &'a SessionView,
}

impl<'a> SessionListView<'a> {
    pub fn new(sidebar: &'a Sidebar, owner: &'a SessionView) -> Self {
        Self { sidebar, owner }
    }
    pub fn render_sessions(
        &self,
        p: Palette,
        _window: &mut Window,
        cx: &mut Context<SessionView>,
    ) -> impl IntoElement {
        let list = &self.sidebar.list();
        let groups = self.sidebar.motion().projects();
        let empty = groups.is_empty();
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div().px_2().child(
                    SessionToolbar::new(self.sidebar, self.owner).render_session_toolbar(p, cx),
                ),
            )
            .when(self.owner.management().is_pending(), |list| {
                list.child(
                    div().px_4().py_1().text_xs().text_color(p.muted()).child("正在更新会话…"),
                )
            })
            .child(
                div()
                    .id("session-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(self.sidebar.scroll())
                    .px_2()
                    .pb_2()
                    .children(groups.into_iter().map(|(cwd, sessions)| {
                        ProjectView::new(self.sidebar, self.owner)
                            .render_project(cwd, &sessions, p, cx)
                    }))
                    .when(list.is_loading(), |list| {
                        list.child(
                            div().p_2().text_xs().text_color(p.muted()).child("正在加载会话…"),
                        )
                    })
                    .when(list.is_unsupported(), |list| {
                        list.child(
                            div()
                                .p_2()
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
                            list.child(div().p_2().text_xs().text_color(p.muted()).child(
                                if self.sidebar.list().show_archived() {
                                    "暂无已归档会话"
                                } else {
                                    "暂无历史会话"
                                },
                            ))
                        },
                    ),
            )
    }
}
