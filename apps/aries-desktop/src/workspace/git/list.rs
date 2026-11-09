use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::git::index::IndexAction;
use crate::workspace::git::panel::GitPanel;
use crate::workspace::git::repository::{ChangeGroup, ChangedFile};

pub struct GitChangeList<'a> {
    panel: &'a GitPanel,
    view: &'a Entity<SessionView>,
    group: ChangeGroup,
    row_height: Pixels,
}

impl<'a> GitChangeList<'a> {
    pub fn new(
        panel: &'a GitPanel,
        view: &'a Entity<SessionView>,
        group: ChangeGroup,
        row_height: Pixels,
    ) -> Self {
        Self { panel, view, group, row_height }
    }

    pub fn render(&self, cx: &App) -> AnyElement {
        let view = self.view;
        let group = self.group;
        let row_height = self.row_height;
        let conversation = self.panel;
        let count = conversation.motion().rows(group).count();
        let live_count = conversation.motion().rows(group).filter(|row| !row.is_exiting()).count();
        let mut panel = div().id(group.id()).flex().flex_col().gap_1().child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!("{} · {live_count}", group.title())),
        );
        if count == 0 {
            return panel.into_any_element();
        }
        let scroll = conversation.motion().scroll(group);
        if conversation.is_collapsed() {
            let now = cx.background_executor().now();
            let offset = scroll.0.borrow().base_handle.offset().y;
            let start = (-offset / row_height).floor() as usize;
            let fraction = offset + row_height * start as f32;
            return panel
                .child(div().h(row_height * count.min(5) as f32).overflow_hidden().child(
                    div().relative().top(fraction).children(
                        conversation.motion().rows(group).skip(start).take(6).map(|row| {
                            div()
                                .h_8()
                                .w_full()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_2()
                                .opacity(row.visibility().sample(now))
                                .child(
                                    div()
                                        .opacity(row.status().sample(now))
                                        .flex_shrink_0()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(
                                            Icon::new(ChangedFile::status_icon(
                                                row.file().status(),
                                            ))
                                            .size_4(),
                                        ),
                                )
                                .child(
                                    div()
                                        .size_6()
                                        .flex_shrink_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(
                                            Icon::new(match group.action() {
                                                IndexAction::Stage => IconName::Plus,
                                                IndexAction::Unstage => IconName::Minus,
                                            })
                                            .size_4(),
                                        ),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_sm()
                                        .child(row.file().path().to_string_lossy().into_owned()),
                                )
                                .child(
                                    div()
                                        .flex_shrink_0()
                                        .opacity(row.lines().sample(now))
                                        .child(row.file().lines().render(cx)),
                                )
                        }),
                    ),
                ))
                .into_any_element();
        }
        let view = view.clone();
        panel = panel.child(
            uniform_list(group.id(), count, move |range, _, cx| {
                let owner = view.read(cx);
                let conversation = owner.git_panel();
                let now = cx.background_executor().now();
                let directory = conversation.directory();
                let busy = conversation.is_index_busy();
                let start = range.start;
                let length = range.len();
                conversation
                    .motion()
                    .rows(group)
                    .skip(start)
                    .take(length)
                    .map(|row| {
                        let file = row.file();
                        let loading = conversation.pending_index().is_some_and(|(path, action)| {
                            path == file.path() && *action == group.action()
                        });
                        let path = SharedString::from(file.path().to_string_lossy().into_owned());
                        let status = ChangedFile::status_kind(file.status());
                        let action = group.action();
                        let action_label = match action {
                            IndexAction::Stage => "添加到暂存区",
                            IndexAction::Unstage => "取消暂存",
                        };
                        let view = view.clone();
                        let target = file.path().to_path_buf();
                        let directory = directory.clone();
                        div()
                            .id(path.clone())
                            .h_8()
                            .w_full()
                            .min_w_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .rounded_md()
                            .opacity(row.visibility().sample(now))
                            .when(!row.is_exiting() && !busy, |row| {
                                row.hover(|style| style.bg(cx.theme().accent))
                            })
                            .aria_label(format!("{path} · {status}"))
                            .child(
                                div()
                                    .id("file-status")
                                    .opacity(row.status().sample(now))
                                    .flex_shrink_0()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        Icon::new(ChangedFile::status_icon(file.status())).size_4(),
                                    )
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(status.label()).build(window, cx)
                                    }),
                            )
                            .child(
                                Button::new("index-action")
                                    .flex_shrink_0()
                                    .ghost()
                                    .small()
                                    .size_6()
                                    .loading(loading)
                                    .icon(match action {
                                        IndexAction::Stage => IconName::Plus,
                                        IndexAction::Unstage => IconName::Minus,
                                    })
                                    .accessibility_label(format!(
                                        "{action_label}：{}",
                                        target.display()
                                    ))
                                    .tooltip(action_label)
                                    .disabled(
                                        (busy && !loading)
                                            || row.is_exiting()
                                            || conversation.is_collapsed()
                                            || file.status().is_conflicted(),
                                    )
                                    .on_click(move |_, window, cx| {
                                        view.update(cx, |this, cx| {
                                            if this.directory() == directory {
                                                this.with_git_panel(|panel| {
                                                    panel.change_git_index(
                                                        target.clone(),
                                                        action,
                                                        window,
                                                        cx,
                                                    );
                                                });
                                            }
                                        });
                                    }),
                            )
                            .child(
                                div()
                                    .id("file-path")
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_sm()
                                    .child(path.clone())
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(path.clone()).build(window, cx)
                                    }),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .opacity(row.lines().sample(now))
                                    .child(file.lines().render(cx)),
                            )
                    })
                    .collect()
            })
            .track_scroll(scroll)
            .h(rems(count.min(5) as f32 * 2.))
            .w_full(),
        );
        panel.into_any_element()
    }
}
