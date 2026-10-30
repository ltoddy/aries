use gpui_kit::component::{ActiveTheme, h_resizable, resizable_panel};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ui::ChatView;
use crate::ui::palette::Palette;

impl ChatView {
    pub fn render_layout(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx.theme().mode);
        div()
            .size_full()
            .flex()
            .bg(p.background())
            .text_color(p.foreground())
            .font_family("Monaco")
            .text_sm()
            .child(
                h_resizable("layout")
                    .child(
                        resizable_panel()
                            .size(px(288.))
                            .size_range(px(200.)..px(520.))
                            .child(self.sidebar(p, cx)),
                    )
                    .child(resizable_panel().child(self.render_main_area(p, cx))),
            )
    }

    pub fn render_main_area(&mut self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(44.))
                    .flex_shrink_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .justify_end()
                    .child(self.render_theme_button(p, cx)),
            )
            .child(self.render_timeline(p, cx))
            .child(self.composer(p, cx))
    }
}
