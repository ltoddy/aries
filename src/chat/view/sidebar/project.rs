use std::path::Path;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::{Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;
use crate::chat::view::sidebar::motion::ProjectBody;

impl ChatView {
    pub fn render_project(&self, cwd: &Path, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let list = &self.session_list;

        let sessions = self.sidebar_motion.project_rows(cwd);
        let expanded = list.is_project_expanded(cwd);
        let count = sessions.len();
        let directory = cwd.file_name().unwrap_or(cwd.as_os_str()).to_string_lossy();
        let path = cwd.to_path_buf();
        let delete_path = path.clone();
        let delete_anchor = format!("project:{}", delete_path.display());
        let has_sessions = sessions.iter().any(|row| !row.is_exiting());
        let delete_enabled = self.can_manage_sessions() && has_sessions;
        let view = cx.weak_entity();
        div()
            .mb_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(SharedString::from(format!("cwd:{}", cwd.display())))
                            .ghost()
                            .small()
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
                                this.session_list.toggle_project(&path);
                                cx.notify();
                            })),
                    )
                    .child(
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
                                .accessibility_label(format!("项目 {} 的更多操作", cwd.display()))
                                .disabled(!delete_enabled)
                                .dropdown_menu_with_anchor(
                                    Anchor::TopRight,
                                    {
                                        let delete_anchor = delete_anchor.clone();
                                        move |menu, _, _| {
                                            let view = view.clone();
                                            let delete_path = delete_path.clone();
                                            let delete_anchor = delete_anchor.clone();
                                            menu.item(
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
                                    },
                                ),
                            )
                            .when_some(
                                self.render_session_delete_popup(delete_anchor, p, cx),
                                |button, popup| button.child(popup),
                            ),
                    ),
            )
            .child(ProjectBody {
                id: SharedString::from(format!("project:{}", cwd.display())).into(),
                expanded,
                child: div()
                    .mt_1()
                    .ml_2()
                    .pl_2()
                    .border_l_1()
                    .border_color(p.border())
                    .children(
                        sessions
                            .into_iter()
                            .map(|row| self.render_session_row(row, expanded, p, cx)),
                    )
                    .into_any_element(),
            })
            .into_any_element()
    }
}
