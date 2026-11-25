use gpui_kit::component::TitleBar;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::workspace_view::{Shell, WorkspaceView};

impl WorkspaceView {
    pub fn render_title_bar(&self, shell: &Shell, cx: &mut Context<Self>) -> AnyElement {
        TitleBar::new()
            .h_9()
            .bg(shell.palette.sidebar().opacity(0.))
            .border_b_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .h_full()
                    .flex_1()
                    .min_w_0()
                    .when(shell.sidebar.is_visible(), |bar| {
                        bar.child(
                            div()
                                .w(shell.sidebar.title_width(shell.sizes))
                                .flex_shrink_0()
                                .px_2()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Aries"),
                        )
                    })
                    .child(self.render_tab_bar(shell.palette, shell.sidebar.available(), cx))
                    .child(
                        div()
                            .w_20()
                            .h_full()
                            .flex_shrink_0()
                            .child(self.fps_counter.clone().cached(shell.style.clone())),
                    )
                    .child(div().w_3().h_full().flex_shrink_0()),
            )
            .into_any_element()
    }
}
