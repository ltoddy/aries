use std::path::Path;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::settings::palette::Palette;
use crate::workspace::sidebar::motion::{SidebarRow, project_body};
use crate::workspace::sidebar::session::SessionRowView;
use crate::workspace::sidebar::{SESSION_LIMIT, Sidebar};

pub struct ProjectView<'a> {
    sidebar: &'a Sidebar,
    owner: &'a SessionView,
}

impl<'a> ProjectView<'a> {
    pub fn new(sidebar: &'a Sidebar, owner: &'a SessionView) -> Self {
        Self { sidebar, owner }
    }
    pub fn render_project(
        &self,
        cwd: impl AsRef<Path>,
        sessions: &[&SidebarRow],
        p: Palette,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) -> AnyElement {
        let cwd = cwd.as_ref();
        let list = &self.sidebar.list();

        let expanded = list.is_project_expanded(cwd);
        let count = sessions.len();
        let sessions_expanded = self.sidebar.is_sessions_expanded(cwd);
        let visible_count = if sessions_expanded { count } else { count.min(SESSION_LIMIT) };
        let directory = cwd.file_name().unwrap_or(cwd.as_os_str()).to_string_lossy();
        let path = cwd.to_owned();
        let delete_path = path.clone();
        let more_path = path.clone();
        let delete_anchor = format!("project:{}", delete_path.display());
        let has_sessions = sessions.iter().any(|row| !row.is_exiting());
        let delete_enabled = self.owner.can_manage_sessions()
            && self.owner.supports_session_deletion()
            && has_sessions;
        let view = cx.weak_entity();
        div()
            .mb_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(format!("cwd:{}", cwd.display())))
                            .ghost()
                            .small()
                            .px_2()
                            .rounded(cx.theme().radius)
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .w_full()
                                    .min_w_0()
                                    .flex()
                                    .items_center()
                                    .justify_start()
                                    .gap_1()
                                    .text_left()
                                    .child(
                                        Icon::new(if expanded {
                                            IconName::FolderOpen
                                        } else {
                                            IconName::Folder
                                        })
                                        .size(rems(0.875))
                                        .flex_shrink_0(),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .child(format!("{directory} ({count})")),
                                    ),
                            )
                            .tooltip(cwd.to_string_lossy().into_owned())
                            .accessibility_label(format!(
                                "{}目录：{}",
                                if expanded { "收起" } else { "展开" },
                                cwd.display()
                            ))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.with_sidebar(|sidebar| {
                                    sidebar.list_mut().toggle_project(&path)
                                });
                                cx.notify();
                            })),
                    )
                    .when(self.owner.supports_session_deletion(), |project| {
                        project.child(
                            div()
                                .relative()
                                .flex_shrink_0()
                                .child(
                                    Button::new(SharedString::from(format!(
                                        "project-menu:{}",
                                        cwd.display()
                                    )))
                                    .icon(Icon::new(IconName::Ellipsis).size(rems(0.8125)))
                                    .ghost()
                                    .small()
                                    .tooltip("项目操作")
                                    .accessibility_label(format!(
                                        "项目 {} 的更多操作",
                                        cwd.display()
                                    ))
                                    .disabled(!delete_enabled)
                                    .dropdown_menu_with_anchor(Anchor::TopRight, {
                                        let delete_anchor = delete_anchor.clone();
                                        move |menu, _, cx| {
                                            let mut menu = menu;
                                            for (days, label) in [
                                                (3, "删除超过三天的会话…"),
                                                (7, "删除超过七天的会话…"),
                                                (15, "删除超过十五天的会话…"),
                                            ] {
                                                let enabled = delete_enabled
                                                    && view.upgrade().is_some_and(|view| {
                                                        !view
                                                            .read(cx)
                                                            .old_session_ids(&delete_path, days)
                                                            .is_empty()
                                                    });
                                                let view = view.clone();
                                                let path = delete_path.clone();
                                                menu = menu.item(
                                                    PopupMenuItem::new(label)
                                                        .icon(IconName::Trash)
                                                        .disabled(!enabled)
                                                        .on_click(move |_, window, cx| {
                                                            let view = view.clone();
                                                            let path = path.clone();
                                                            window.defer(cx, move |window, cx| {
                                                                let _ =
                                                                    view.update(cx, |this, cx| {
                                                                        this.delete_old_sessions(
                                                                            &path, days, window, cx,
                                                                        );
                                                                    });
                                                            });
                                                        }),
                                                );
                                            }
                                            let view = view.clone();
                                            let delete_path = delete_path.clone();
                                            let delete_anchor = delete_anchor.clone();
                                            menu.item(PopupMenuItem::separator()).item(
                                                PopupMenuItem::new("删除项目会话")
                                                    .icon(IconName::Trash)
                                                    .disabled(!delete_enabled)
                                                    .on_click(move |_, _, cx| {
                                                        let _ = view.update(cx, |this, cx| {
                                                            let ids = this
                                                                .session_ids(Some(&delete_path));
                                                            this.prepare_session_deletion(
                                                                ids,
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
                                    self.owner.render_session_delete_popup(&delete_anchor, p, cx),
                                    |button, popup| button.child(popup),
                                ),
                        )
                    }),
            )
            .child(project_body(
                SharedString::from(format!("project:{}", cwd.display())).into(),
                expanded,
                window,
                cx,
                |cx| {
                    div()
                        .children(sessions.iter().take(visible_count).map(|row| {
                            SessionRowView::new(self.sidebar, self.owner)
                                .render_session_row(row, expanded, p, cx)
                        }))
                        .when(count > SESSION_LIMIT, |body| {
                            body.child(
                                Button::new(SharedString::from(format!(
                                    "more-sessions:{}",
                                    cwd.display()
                                )))
                                .ghost()
                                .small()
                                .w_full()
                                .text_color(p.muted())
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .pl(rems(1.125))
                                        .w_full()
                                        .min_w_0()
                                        .child(div().flex_shrink_0().child(if sessions_expanded {
                                            "收起"
                                        } else {
                                            "展开更多"
                                        })),
                                )
                                .accessibility_label(format!(
                                    "{}目录 {} 的会话",
                                    if sessions_expanded { "收起" } else { "展开更多" },
                                    cwd.display()
                                ))
                                .disabled(!expanded)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.with_sidebar(|sidebar| {
                                            sidebar.toggle_sessions(&more_path)
                                        });
                                        cx.notify();
                                    },
                                )),
                            )
                        })
                        .into_any_element()
                },
            ))
            .into_any_element()
    }
}
