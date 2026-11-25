use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants, Toggle};
use gpui_kit::component::{Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::settings::palette::Palette;

pub fn render_toolbar(
    owner: &SessionView,
    palette: Palette,
    cx: &mut Context<SessionView>,
) -> impl IntoElement {
    let list = &owner.sidebar().list();
    div()
        .flex()
        .items_center()
        .justify_between()
        .flex_shrink_0()
        .pl_2()
        .mb_1()
        .text_xs()
        .text_color(palette.muted())
        .child("对话")
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .when(owner.management().capabilities().archive.is_some(), |toolbar| {
                    toolbar.child(
                        Toggle::new("show-archived-sessions")
                            .label("已归档")
                            .checked(list.show_archived())
                            .small()
                            .disabled(
                                owner.management().is_pending()
                                    || owner.deletion().is_busy()
                                    || !list.is_supported(),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.with_sidebar(|sidebar| sidebar.toggle_archived());
                                this.refresh_sessions(cx);
                                this.sync_sidebar_rows(cx);
                                cx.notify();
                            })),
                    )
                })
                .child(
                    Button::new("expand-all-sessions")
                        .icon(Icon::new(IconName::ListChevronsUpDown).size(rems(0.875)))
                        .tooltip("展开全部")
                        .accessibility_label("展开全部项目")
                        .ghost()
                        .small()
                        .text_color(palette.muted())
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
                        .text_color(palette.muted())
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
                        .text_color(palette.muted())
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
                        .text_color(palette.muted())
                        .disabled(
                            list.is_loading()
                                || !list.is_supported()
                                || owner.execution().is_disconnected()
                                || owner.deletion().is_busy()
                                || owner.management().is_pending()
                                || owner.execution().is_loading(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.refresh_sessions(cx))),
                ),
        )
}
