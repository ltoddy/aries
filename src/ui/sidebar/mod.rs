mod list;
mod project;
mod session;
mod toolbar;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::{Disableable, Icon};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ui::ChatView;
use crate::ui::palette::Palette;

impl ChatView {
    pub fn sidebar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("sidebar")
            .w_full()
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .bg(p.sidebar())
            .child(
                div().p_3().child(
                    Button::new("new-chat")
                        .label("新建对话")
                        .icon(Icon::new(IconName::Plus).size(px(15.)))
                        .w_full()
                        .h(px(38.))
                        .rounded(px(12.))
                        .disabled(!self.can_start_new_chat())
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.render_mode == crate::ui::ChatRenderMode::Sidebar
                                && this.tab_host.is_some()
                            {
                                this.request_new_tab(cx);
                            } else {
                                this.new_chat(window, cx);
                            }
                        })),
                ),
            )
            .child(self.render_sessions(p, cx))
    }
}
