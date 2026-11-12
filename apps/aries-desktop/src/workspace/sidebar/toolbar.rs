use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::settings::palette::Palette;
use crate::workspace::sidebar::Sidebar;

pub struct SessionToolbar<'a> {
    sidebar: &'a Sidebar,
    owner: &'a SessionView,
}

impl<'a> SessionToolbar<'a> {
    pub fn new(sidebar: &'a Sidebar, owner: &'a SessionView) -> Self {
        Self { sidebar, owner }
    }
    pub fn render_session_toolbar(
        &self,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> impl IntoElement {
        let list = &self.sidebar.list();
        div()
            .flex()
            .items_center()
            .justify_between()
            .flex_shrink_0()
            .pl_2()
            .mb_1()
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
                            .text_color(p.muted())
                            .on_click(cx.listener(|this, _, _, cx| {
                                let directory = this.directory().to_owned();
                                this.with_sidebar(|sidebar| {
                                    sidebar.list_mut().expand_all();
                                    sidebar.list_mut().expand_project(&directory);
                                });
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
                            .text_color(p.muted())
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.with_sidebar(|sidebar| sidebar.collapse_all());
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
                            .text_color(p.muted())
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
                            .text_color(p.muted())
                            .disabled(
                                list.is_loading()
                                    || !list.is_supported()
                                    || self.owner.execution().is_disconnected()
                                    || self.owner.deletion().is_busy()
                                    || self.owner.execution().is_loading(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_sessions(cx))),
                    ),
            )
    }
}
