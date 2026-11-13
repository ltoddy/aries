use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonCustomVariant, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::session::SessionOperation;
use crate::workspace::settings::palette::Palette;
use crate::workspace::sidebar::Sidebar;
use crate::workspace::sidebar::motion::{SessionSurface, SidebarRow};

pub struct SessionRowView<'a> {
    sidebar: &'a Sidebar,
    owner: &'a SessionView,
}

impl<'a> SessionRowView<'a> {
    pub fn new(sidebar: &'a Sidebar, owner: &'a SessionView) -> Self {
        Self { sidebar, owner }
    }
    pub fn render_session_row(
        &self,
        row: &SidebarRow,
        expanded: bool,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> AnyElement {
        let session = row.session();
        let enabled = expanded && !row.is_exiting();
        let id = session.session_id.to_string();
        let execution = self.owner.execution();
        let current = execution.session_id() == Some(&session.session_id);
        let loading = self.sidebar.loading_session() == Some(&session.session_id)
            || (execution.is_loading() && execution.loading_id() == Some(&session.session_id));
        let title = session
            .title
            .as_ref()
            .filter(|title| !title.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| if current { execution.conversation().title() } else { id.clone() });
        let display_title = crate::workspace::conversation::single_line(&title);
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
        let can_open_session = !session.archived.unwrap_or(false)
            && !self.owner.management().is_pending()
            && self.owner.execution().supports_load()
            && !self.owner.execution().is_disconnected()
            && (self.owner.tab_host().is_some() || self.owner.can_load_sessions());
        let capabilities = self.owner.session_management_capabilities(&session.session_id, cx);
        let has_actions = capabilities.title.is_some()
            || capabilities.fork.is_some()
            || capabilities.archive.is_some()
            || capabilities.delete.is_some();
        let view = cx.weak_entity();
        let selected = current
            && !self
                .owner
                .tab_host()
                .and_then(|host| host.upgrade())
                .is_some_and(|host| host.read(cx).is_usage_visible());
        SessionSurface::new(
            row.clone(),
            selected,
            cx.theme().sidebar_accent,
            div()
                .id(SharedString::from(format!("session:{id}")))
                .when(current, |row| row.anchor_scroll(Some(self.sidebar.anchor().clone())))
                .pl_2()
                .tooltip(move |window, cx| Tooltip::new(details.clone()).build(window, cx))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Button::new(SharedString::from(format!("load-session:{id}")))
                                .custom(
                                    ButtonCustomVariant::new(cx).active(cx.theme().sidebar_accent),
                                )
                                .small()
                                .px_0()
                                .selected(selected)
                                .bg(cx.theme().transparent)
                                .flex_1()
                                .min_w_0()
                                .disabled(!enabled || !can_open_session)
                                .accessibility_label(format!("打开会话 {title}"))
                                .when(loading, |button| {
                                    button.child(
                                        div().text_xs().text_color(p.muted()).child("加载中…"),
                                    )
                                })
                                .child(
                                    div()
                                        .w_full()
                                        .min_w_0()
                                        .pl(rems(1.125))
                                        .text_left()
                                        .truncate()
                                        .font_weight(if selected {
                                            FontWeight::MEDIUM
                                        } else {
                                            FontWeight::NORMAL
                                        })
                                        .child(display_title),
                                )
                                .on_click(cx.listener(move |this, _, _window, cx| {
                                    let session = load_session.clone();
                                    if this.tab_host().is_some() {
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
                                .when(has_actions, |actions| actions.child(
                                    Button::new(SharedString::from(format!("session-menu:{id}")))
                                        .icon(Icon::new(IconName::Ellipsis).size(rems(0.8125)))
                                        .ghost()
                                        .small()
                                        .text_color(p.muted())
                                        .tooltip("会话操作")
                                        .accessibility_label(action_label)
                                        .disabled(!enabled || !self.owner.can_manage_sessions())
                                        .dropdown_menu_with_anchor(Anchor::TopRight, {
                                            let delete_anchor = delete_anchor.clone();
                                            let session = session.clone();
                                            move |mut menu, _, cx| {
                                                let Some(owner) = view.upgrade() else { return menu; };
                                                let owner = owner.read(cx);
                                                let caps = owner.session_management_capabilities(&session.session_id, cx);
                                                let available = enabled && owner.management_available(&session.session_id, cx);
                                                for (label, operation, supported) in [
                                                    ("重命名…", SessionOperation::title(), caps.title.is_some()),
                                                    ("复制会话", SessionOperation::fork(), caps.fork.is_some()),
                                                    (if session.archived.unwrap_or(false) { "恢复会话" } else { "归档会话" }, SessionOperation::archive(!session.archived.unwrap_or(false)), caps.archive.is_some()),
                                                ] {
                                                    if !supported { continue; }
                                                    let view = view.clone();
                                                    let session = session.clone();
                                                    menu = menu.item(PopupMenuItem::new(label).disabled(!available).on_click(move |_, window, cx| {
                                                        let view = view.clone(); let session = session.clone();
                                                        window.defer(cx, move |window, cx| {
                                                            let _ = view.update(cx, |this, cx| match operation {
                                                                SessionOperation::Title => this.rename_session(session, window, cx),
                                                                SessionOperation::Fork => this.fork_session(session, cx),
                                                                SessionOperation::Archive(_) => this.archive_session(session, cx),
                                                            });
                                                        });
                                                    }));
                                                }
                                                if owner.supports_session_deletion() {
                                                    if caps.title.is_some() || caps.fork.is_some() || caps.archive.is_some() {
                                                        menu = menu.separator();
                                                    }
                                                    let view = view.clone();
                                                    let delete_id = delete_id.clone();
                                                    let delete_anchor = delete_anchor.clone();
                                                    menu = menu.item(PopupMenuItem::new("删除会话").icon(IconName::Trash).disabled(!available).on_click(move |_, _, cx| {
                                                        let _ = view.update(cx, |this, cx| this.prepare_session_deletion(vec![delete_id.clone()], delete_anchor.clone(), cx));
                                                    }));
                                                }
                                                menu
                                            }
                                        }),
                                ))
                                .when_some(
                                    enabled
                                        .then(|| {
                                            self.owner.render_session_delete_popup(
                                                &delete_anchor,
                                                p,
                                                cx,
                                            )
                                        })
                                        .flatten(),
                                    |button, popup| button.child(popup),
                                ),
                        ),
                )
                .into_any_element(),
        )
        .into_any_element()
    }
}
