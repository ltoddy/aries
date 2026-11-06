use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;

impl ChatView {
    pub fn locate_current_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self
            .session_id
            .as_ref()
            .and_then(|id| self.session_list.session(id))
            .map(|session| session.cwd.clone())
            .unwrap_or_else(|| self.cwd.clone());
        self.session_list.expand_project(&cwd);
        let anchor = self.current_session_anchor.clone();
        window.on_next_frame(move |window, cx| {
            anchor.scroll_to(window, cx);
            window.on_next_frame(|window, _| window.refresh());
        });
        cx.notify();
    }

    pub fn render_session_toolbar(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let list = &self.session_list;
        div()
            .flex()
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .pl_2()
            .mb_2()
            .text_xs()
            .text_color(p.muted())
            .child("对话")
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("expand-all-sessions")
                            .icon(Icon::new(IconName::ListChevronsUpDown).size(rems(0.875)))
                            .tooltip("展开全部")
                            .accessibility_label("展开全部项目")
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.session_list.expand_all();
                                this.session_list.expand_project(&this.cwd);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("collapse-all-sessions")
                            .icon(Icon::new(IconName::ListChevronsDownUp).size(rems(0.875)))
                            .tooltip("收起全部")
                            .accessibility_label("收起全部项目")
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.session_list.collapse_all();
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("locate-current-session")
                            .icon(Icon::new(IconName::LocateFixed).size(rems(0.875)))
                            .tooltip("定位当前会话")
                            .accessibility_label("定位当前会话")
                            .ghost()
                            .small()
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.locate_current_session(window, cx);
                            })),
                    )
                    .child(
                        Button::new("refresh-sessions")
                            .icon(Icon::new(IconName::RefreshCw).size(rems(0.875)))
                            .tooltip("重新获取会话列表")
                            .accessibility_label("刷新会话列表")
                            .ghost()
                            .small()
                            .disabled(
                                list.is_loading()
                                    || !list.is_supported()
                                    || self.disconnected
                                    || self.session_deletion.is_busy()
                                    || self.loading_session.is_some(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_sessions(cx))),
                    ),
            )
    }
}
