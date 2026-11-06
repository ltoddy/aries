use agent_client_protocol::schema::v1::SessionInfo;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::ui::ChatView;
use crate::ui::palette::Palette;

impl ChatView {
    pub fn render_current_session(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        if let Some(id) = &self.session_id {
            return self
                .render_session_row(
                    &SessionInfo::new(id.clone(), self.cwd.clone()).title(self.chat.title()),
                    p,
                    cx,
                )
                .into_any_element();
        }
        let title = self.chat.title();
        let display_title = crate::ui::single_line(&title);
        div()
            .id("current-chat")
            .anchor_scroll(Some(self.current_session_anchor.clone()))
            .px_2()
            .py_1()
            .mb_1()
            .bg(p.hover())
            .child(div().truncate().font_weight(FontWeight::MEDIUM).child(display_title))
            .child(div().mt_1().text_xs().text_color(p.muted()).child("发送消息开始对话"))
            .into_any_element()
    }

    pub fn render_session_row(
        &self,
        session: &SessionInfo,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = session.session_id.to_string();
        let current = self.session_id.as_ref() == Some(&session.session_id);
        let loading = self
            .loading_session
            .as_ref()
            .is_some_and(|loading| loading.session_id() == &session.session_id);
        let title = session
            .title
            .as_ref()
            .filter(|title| !title.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| if current { self.chat.title() } else { id.clone() });
        let display_title = crate::ui::single_line(&title);
        let details = format!(
            "{}\n{}\n{}{}",
            title,
            session.cwd.display(),
            id,
            session
                .updated_at
                .as_ref()
                .map(|time| format!("\n更新时间：{time}"))
                .unwrap_or_default()
        );
        let delete_id = session.session_id.clone();
        let action_label = format!("会话 {title} 的更多操作");
        let delete_anchor = format!("session:{}", delete_id);
        let load_session = session.clone();
        let can_open_session = self.tab_host.is_some() || self.can_load_sessions();
        let view = cx.weak_entity();
        div()
            .id(SharedString::from(format!("session:{id}")))
            .when(current, |row| row.anchor_scroll(Some(self.current_session_anchor.clone())))
            .pl_2()
            .py_1()
            .mb_1()
            .when(current || loading, |row| row.bg(p.hover()))
            .tooltip(move |window, cx| Tooltip::new(details.clone()).build(window, cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new(SharedString::from(format!("load-session:{id}")))
                            .ghost()
                            .small()
                            .h_auto()
                            .px_0()
                            .flex_1()
                            .min_w_0()
                            .disabled(!can_open_session)
                            .accessibility_label(format!("打开会话 {title}"))
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .text_left()
                                    .truncate()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(display_title),
                            )
                            .on_click(cx.listener(move |this, _, _window, cx| {
                                let session = load_session.clone();
                                if this.tab_host.is_some() {
                                    this.request_session_tab(session, cx);
                                } else {
                                    this.load_session(session, cx);
                                }
                            })),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_shrink_0()
                            .child(
                                Button::new(SharedString::from(format!("session-menu:{id}")))
                                    .icon(Icon::new(IconName::Ellipsis).size(rems(0.8125)))
                                    .ghost()
                                    .small()
                                    .tooltip("会话操作")
                                    .accessibility_label(action_label)
                                    .disabled(!self.can_manage_sessions())
                                    .dropdown_menu_with_anchor(Anchor::TopRight, {
                                        let delete_anchor = delete_anchor.clone();
                                        move |menu, _, _| {
                                            let view = view.clone();
                                            let delete_id = delete_id.clone();
                                            let delete_anchor = delete_anchor.clone();
                                            menu.item(
                                                PopupMenuItem::new("删除会话")
                                                    .icon(IconName::Trash)
                                                    .on_click(move |_, _, cx| {
                                                        let _ = view.update(cx, |this, cx| {
                                                            this.prepare_session_deletion(
                                                                vec![delete_id.clone()],
                                                                delete_anchor.clone(),
                                                                cx,
                                                            );
                                                        });
                                                    }),
                                            )
                                        }
                                    }),
                            )
                            .when_some(
                                self.render_session_delete_popup(delete_anchor, p, cx),
                                |button, popup| button.child(popup),
                            ),
                    ),
            )
            .into_any_element()
    }
}
