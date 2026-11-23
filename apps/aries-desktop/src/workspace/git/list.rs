use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Selectable, Sizable};
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
}

impl<'a> GitChangeList<'a> {
    pub fn new(panel: &'a GitPanel, view: &'a Entity<SessionView>, group: ChangeGroup) -> Self {
        Self { panel, view, group }
    }

    pub fn render(&self, cx: &App) -> AnyElement {
        let view = self.view;
        let group = self.group;
        let conversation = self.panel;
        let count = conversation.motion().rows(group).count();
        let live_count = conversation.motion().rows(group).filter(|row| !row.is_exiting()).count();
        let title = div()
            .flex_shrink_0()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!("{} · {live_count}", group.title()));
        if count == 0 {
            return div().flex_shrink_0().flex().flex_col().gap_1().child(title).into_any_element();
        }
        let scroll = conversation.motion().scroll(group);
        let view = view.clone();
        div()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(title)
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(
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
                                    let loading = conversation.pending_index().is_some_and(
                                        |(path, action)| {
                                            path == file.path() && *action == group.action()
                                        },
                                    );
                                    let path = SharedString::from(
                                        file.path().to_string_lossy().into_owned(),
                                    );
                                    let status = ChangedFile::status_kind(file.status());
                                    let action = group.action();
                                    let action_label = match action {
                                        IndexAction::Stage => "添加到暂存区",
                                        IndexAction::Unstage => "取消暂存",
                                    };
                                    let view = view.clone();
                                    let target = file.path().to_owned();
                                    let directory = directory.to_owned();
                                    let diff_view = view.clone();
                                    let diff_target = target.clone();
                                    let diff_directory = directory.clone();
                                    let selected = conversation
                                        .diff()
                                        .read(cx)
                                        .is_selected(file.path(), group);
                                    div()
                                        .id(path.clone())
                                        .h_8()
                                        .w_full()
                                        .min_w_0()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .px_1()
                                        .rounded(cx.theme().radius_tokens().md)
                                        .when(selected, |row| {
                                            row.bg(cx.theme().foreground.opacity(0.115))
                                        })
                                        .opacity(row.visibility().sample(now))
                                        .when(!row.is_exiting() && !busy, |row| {
                                            row.hover(|style| {
                                                style
                                                    .bg(cx.theme().foreground.opacity(
                                                        if selected { 0.115 } else { 0.07 },
                                                    ))
                                            })
                                        })
                                        .aria_label(format!("{path} · {status}"))
                                        .child(
                                            div()
                                                .id("file-status")
                                                .opacity(row.status().sample(now))
                                                .flex_shrink_0()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(
                                                    Icon::new(ChangedFile::status_icon(
                                                        file.status(),
                                                    ))
                                                    .size_4(),
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
                                            Button::new("file-path")
                                                .ghost()
                                                .small()
                                                .flex_1()
                                                .min_w_0()
                                                .px_0()
                                                .selected(selected)
                                                .bg(cx.theme().transparent)
                                                .text_color(cx.theme().foreground)
                                                .when(selected, |button| {
                                                    button.font_weight(FontWeight::MEDIUM)
                                                })
                                                .disabled(row.is_exiting())
                                                .accessibility_label(format!(
                                                    "查看 diff：{path} · {}",
                                                    group.title()
                                                ))
                                                .tooltip(&path)
                                                .child(
                                                    div()
                                                        .w_full()
                                                        .truncate()
                                                        .text_left()
                                                        .child(path),
                                                )
                                                .on_click(move |_, window, cx| {
                                                    diff_view.update(cx, |this, cx| {
                                                        if this.directory() == diff_directory {
                                                            this.with_git_panel(|panel| {
                                                                panel.open_diff(
                                                                    diff_target.clone(),
                                                                    group,
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
                                                .flex_shrink_0()
                                                .opacity(row.lines().sample(now))
                                                .child(file.lines().render(cx)),
                                        )
                                })
                                .collect()
                        })
                        .track_scroll(scroll)
                        .size_full(),
                    )
                    .vertical_scrollbar(scroll),
            )
            .into_any_element()
    }
}
