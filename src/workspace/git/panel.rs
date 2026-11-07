use std::path::PathBuf;
use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::git::index::{IndexAction, update_index};
use crate::workspace::git::list::GitChangeList;
use crate::workspace::git::motion::{GitMotion, accept_result};
use crate::workspace::git::repository::{ChangeGroup, GitInfo};
use crate::workspace::git::watch::GitWatch;

pub struct GitPanel {
    cwd: PathBuf,
    info: Option<GitInfo>,
    info_task: Option<Task<()>>,
    refresh_task: Option<Task<()>>,
    refreshing: bool,
    refresh_pending: bool,
    watch_error: Option<String>,
    index_busy: bool,
    revision: u64,
    info_collapsed: bool,
    motion: GitMotion,
    pending_index: Option<(PathBuf, IndexAction)>,
    read_error: Option<String>,
    trigger_focus: FocusHandle,
    panel_focus: FocusHandle,
}

impl GitPanel {
    pub fn new(cwd: PathBuf, cx: &mut Context<SessionView>) -> Self {
        Self {
            cwd,
            info: None,
            info_task: None,
            refresh_task: None,
            refreshing: false,
            refresh_pending: false,
            watch_error: None,
            index_busy: false,
            revision: 0,
            info_collapsed: true,
            motion: GitMotion::default(),
            pending_index: None,
            read_error: None,
            trigger_focus: cx.focus_handle().tab_stop(true),
            panel_focus: cx.focus_handle().tab_stop(true),
        }
    }
    pub fn set_directory(&mut self, cwd: PathBuf) {
        self.cwd = cwd;
    }
    pub fn directory(&self) -> &PathBuf {
        &self.cwd
    }
    pub fn motion(&self) -> &GitMotion {
        &self.motion
    }
    pub fn is_collapsed(&self) -> bool {
        self.info_collapsed
    }
    pub fn is_index_busy(&self) -> bool {
        self.index_busy
    }
    pub fn pending_index(&self) -> Option<&(PathBuf, IndexAction)> {
        self.pending_index.as_ref()
    }
    pub fn info(&self) -> Option<&GitInfo> {
        self.info.as_ref()
    }
    pub fn cleanup(&mut self, cx: &App) {
        self.motion.cleanup(cx.background_executor().now(), cx.reduce_motion());
    }
    pub fn render(
        &self,
        view: &Entity<SessionView>,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyElement> {
        let conversation = self;
        if conversation.info.is_none()
            && conversation.read_error.is_none()
            && conversation.watch_error.is_none()
        {
            return None;
        }
        let summary = conversation
            .read_error
            .as_ref()
            .map(|error| format!("无法读取 Git：{error}"))
            .unwrap_or_else(|| {
                conversation.info.as_ref().map(GitInfo::summary).unwrap_or_default()
            });
        let now = cx.background_executor().now();
        let expanded = !conversation.info_collapsed;
        let progress = conversation.motion.panel().sample(now);
        if conversation.motion.panel().active(now)
            || conversation.motion.summary().active(now)
            || conversation.motion.all_rows().iter().any(|row| {
                row.visibility().active(now) || row.status().active(now) || row.lines().active(now)
            })
        {
            window.request_animation_frame();
        }
        let id: SharedString =
            format!("project-git-{:?}-{}", view.entity_id(), conversation.cwd.display()).into();
        let ball = if progress < 1. || !expanded {
            let view = view.clone();
            Some(
                div()
                    .id((ElementId::from(id.clone()), "ball"))
                    .absolute()
                    .top_3()
                    .right_3()
                    .rounded_full()
                    .shadow_lg()
                    .opacity(1. - progress)
                    .when(!expanded, |ball| ball.occlude())
                    .child(if expanded {
                        div()
                            .size_10()
                            .rounded_full()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().popover)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::GitBranch).size_4())
                            .into_any_element()
                    } else {
                        Button::new("expand-git-info")
                            .outline()
                            .size_10()
                            .rounded_full()
                            .bg(cx.theme().popover)
                            .text_color(cx.theme().popover_foreground)
                            .icon(IconName::GitBranch)
                            .track_focus(&conversation.trigger_focus)
                            .disabled(expanded)
                            .accessibility_label("展开 Git 变更")
                            .tooltip(format!("展开 Git 变更 · {summary}"))
                            .on_click(move |_, window, cx| {
                                view.update(cx, |this, cx| {
                                    this.with_git_panel(|panel| {
                                        panel.set_collapsed(false, window, cx)
                                    })
                                });
                            })
                            .into_any_element()
                    })
                    .into_any_element(),
            )
        } else {
            None
        };
        if progress == 0. && !expanded {
            return ball;
        }
        let mut panel = div()
            .id(id)
            .absolute()
            .top_3()
            .right_3()
            .w(rems(20.))
            .max_w(relative(0.9))
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .text_color(cx.theme().popover_foreground)
            .shadow_lg()
            .opacity(progress)
            .when(expanded, |panel| panel.occlude())
            .mt(cx.theme().motion_tokens().distance_short * (1. - progress))
            .flex()
            .flex_col()
            .gap_3()
            .min_w_0()
            .aria_label("Git 变更文件")
            .when(expanded, |panel| {
                let view = view.clone();
                panel.on_key_down(move |event, window, cx| {
                    if event.keystroke.key == "escape" {
                        view.update(cx, |this, cx| {
                            this.with_git_panel(|panel| panel.set_collapsed(true, window, cx))
                        });
                        cx.stop_propagation();
                    }
                })
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_sm().font_weight(FontWeight::SEMIBOLD).child("Git 变更"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when(expanded, |actions| {
                                let view = view.clone();
                                actions.child(
                                    Button::new("refresh-git-info")
                                        .ghost()
                                        .small()
                                        .icon(IconName::RefreshCw)
                                        .loading(conversation.refreshing)
                                        .disabled(conversation.index_busy)
                                        .accessibility_label("刷新 Git 变更")
                                        .tooltip("刷新 Git 变更")
                                        .on_click(move |_, _, cx| {
                                            view.update(cx, |this, cx| {
                                                this.with_git_panel(|panel| panel.watch_info(cx));
                                            });
                                        }),
                                )
                            })
                            .child(if !expanded {
                                div()
                                    .size_6()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(Icon::new(IconName::ChevronUp).size_4())
                                    .into_any_element()
                            } else {
                                let view = view.clone();
                                let label = "缩小为悬浮球";
                                Button::new("toggle-git-info")
                                    .track_focus(&conversation.panel_focus)
                                    .disabled(!expanded)
                                    .ghost()
                                    .small()
                                    .icon(IconName::ChevronUp)
                                    .accessibility_label(label)
                                    .tooltip(label)
                                    .on_click(move |_, window, cx| {
                                        view.update(cx, |this, cx| {
                                            this.with_git_panel(|panel| {
                                                panel.set_collapsed(true, window, cx)
                                            })
                                        });
                                    })
                                    .into_any_element()
                            }),
                    ),
            );
        let summary_opacity = conversation.motion.summary().sample(now);
        panel = panel.child(
            div()
                .relative()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(div().whitespace_normal().opacity(summary_opacity).child(summary))
                .when_some(conversation.motion.previous_summary(), |summary, previous| {
                    summary.child(
                        div()
                            .absolute()
                            .top_0()
                            .left_0()
                            .w_full()
                            .whitespace_normal()
                            .opacity(1. - summary_opacity)
                            .child(previous.clone()),
                    )
                }),
        );
        if let Some(error) = &conversation.watch_error {
            panel = panel.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().warning)
                    .whitespace_normal()
                    .child(format!("自动刷新不可用：{error}。请手动刷新重试。")),
            );
        }
        if !conversation.motion.all_rows().is_empty() {
            panel = panel
                .child(
                    GitChangeList::new(self, view, ChangeGroup::Staged, window.rem_size() * 2.)
                        .render(cx),
                )
                .child(
                    GitChangeList::new(self, view, ChangeGroup::Unstaged, window.rem_size() * 2.)
                        .render(cx),
                );
        }
        Some(
            div()
                .absolute()
                .inset_0()
                .children([Some(panel.into_any_element()), ball].into_iter().flatten())
                .into_any_element(),
        )
    }

    pub fn set_collapsed(
        &mut self,
        collapsed: bool,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if self.info_collapsed == collapsed {
            return;
        }
        self.info_collapsed = collapsed;
        let tokens = cx.theme().motion_tokens();
        let duration = if cx.reduce_motion() { Duration::ZERO } else { tokens.duration_fast };
        self.motion.panel_mut().retarget(
            if collapsed { 0. } else { 1. },
            cx.background_executor().now(),
            duration,
            if collapsed { tokens.easing_exit.clone() } else { tokens.easing_enter.clone() },
        );
        if collapsed {
            self.trigger_focus.focus(window, cx);
        } else {
            self.panel_focus.focus(window, cx);
        }
        cx.notify();
    }

    pub fn sync_info(
        &mut self,
        result: anyhow::Result<Option<GitInfo>>,
        animate: bool,
        cx: &mut Context<SessionView>,
    ) {
        if self.motion.reset(&self.cwd) {
            self.info = None;
            self.read_error = None;
            self.info_collapsed = true;
        }
        let (info, error) = match result {
            Ok(None) if self.info.is_some() || self.read_error.is_some() => {
                (None, Some("项目不再是可读取的 Git 仓库".into()))
            }
            Ok(info) => (info, None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        let changed = self.info != info || self.read_error != error;
        let tokens = cx.theme().motion_tokens();
        let duration = if cx.reduce_motion() { Duration::ZERO } else { tokens.duration_fast };
        self.motion.sync(
            info.as_ref(),
            cx.background_executor().now(),
            duration,
            tokens.easing_enter.clone(),
            animate,
        );
        self.info = info;
        self.read_error = error;
        if changed {
            if !duration.is_zero() {
                cx.spawn(async move |view, cx| {
                    cx.background_executor().timer(duration).await;
                    let _ = view.update(cx, |this, cx| {
                        this.with_git_panel(|panel| panel.cleanup(cx));
                        cx.notify();
                    });
                })
                .detach();
            }
            cx.notify();
        }
    }

    pub fn change_git_index(
        &mut self,
        path: PathBuf,
        action: IndexAction,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let group = match action {
            IndexAction::Stage => ChangeGroup::Unstaged,
            IndexAction::Unstage => ChangeGroup::Staged,
        };
        if self.index_busy
            || self.info_collapsed
            || !self.motion.rows(group).any(|row| {
                row.file().path() == path
                    && !row.is_exiting()
                    && !row.file().status().is_conflicted()
            })
        {
            return;
        }
        self.index_busy = true;
        self.pending_index = Some((path.clone(), action));
        self.revision += 1;
        let revision = self.revision;
        let directory = self.cwd.clone();
        let action_focus = window.focused(cx);
        cx.spawn_in(window, async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let result = update_index(&directory, &path, action);
                    let info = GitInfo::read(&directory);
                    (result, info, path, directory)
                })
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.with_git_panel(|panel| {
                    panel.index_busy = false;
                    panel.pending_index = None;
                    let current = accept_result(&panel.cwd, panel.revision, &result.3, revision);
                    if current {
                        panel.sync_info(result.1, result.0.is_ok(), cx);
                        if !panel.info_collapsed
                            && window.focused(cx) == action_focus
                            && !panel
                                .motion
                                .rows(group)
                                .any(|row| row.file().path() == result.2 && !row.is_exiting())
                        {
                            panel.panel_focus.focus(window, cx);
                        }
                    }
                    panel.revision += 1;
                    if panel.refresh_pending || !current {
                        panel.refresh_info(cx);
                    }
                    if let Err(error) = result.0 {
                        let label = match action {
                            IndexAction::Stage => "无法暂存",
                            IndexAction::Unstage => "无法取消暂存",
                        };
                        window.push_notification(
                            Notification::error(format!(
                                "{label} {}：{:#}",
                                result.2.display(),
                                anyhow::Error::from(error)
                            )),
                            cx,
                        );
                    }
                    cx.notify();
                });
            });
        })
        .detach();
        cx.notify();
    }

    pub fn watch_info(&mut self, cx: &mut Context<SessionView>) {
        if self.motion.reset(&self.cwd) {
            self.info = None;
            self.read_error = None;
            self.info_collapsed = true;
        }
        self.revision += 1;
        self.refresh_task = None;
        self.refreshing = false;
        self.refresh_pending = false;
        self.watch_error = None;
        let directory = self.cwd.clone();
        self.info_task = Some(cx.spawn(async move |view, cx| {
            let (sender, receiver) = async_channel::bounded(1);
            let executor = cx.background_executor().clone();
            let watcher = cx
                .background_spawn(async move { GitWatch::start(&directory, sender, executor) })
                .await;
            let watcher = match watcher {
                Ok(watcher) => watcher,
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.with_git_panel(|panel| {
                            panel.watch_error = Some(format!("{error:#}"));
                            panel.refresh_info(cx);
                        });
                    });
                    return;
                }
            };
            if view
                .update(cx, |this, cx| this.with_git_panel(|panel| panel.refresh_info(cx)))
                .is_err()
            {
                return;
            }
            while receiver.recv().await.is_ok() {
                cx.background_executor().timer(Duration::from_millis(300)).await;
                while receiver.try_recv().is_ok() {}
                let error = watcher.take_error();
                if view
                    .update(cx, |this, cx| {
                        this.with_git_panel(|panel| {
                            if let Some(error) = error {
                                panel.watch_error =
                                    Some(format!("{:#}", anyhow::Error::from(error)));
                            }
                            panel.refresh_info(cx);
                        });
                    })
                    .is_err()
                {
                    return;
                }
            }
            drop(watcher);
        }));
        cx.notify();
    }

    pub fn refresh_info(&mut self, cx: &mut Context<SessionView>) {
        if self.refreshing || self.index_busy {
            self.refresh_pending = true;
            return;
        }
        self.refreshing = true;
        self.refresh_pending = false;
        self.refresh_task = Some(cx.spawn(async move |view, cx| {
            loop {
                let Ok((directory, revision)) = view.read_with(cx, |this, _| {
                    (this.git_panel().cwd.clone(), this.git_panel().revision)
                }) else {
                    return;
                };
                let (info, directory) = cx
                    .background_spawn(async move { (GitInfo::read(&directory), directory) })
                    .await;
                let again = view.update(cx, |this, cx| {
                    this.with_git_panel(|panel| {
                        if accept_result(&panel.cwd, panel.revision, &directory, revision)
                            && !panel.index_busy
                        {
                            panel.sync_info(info, true, cx);
                        }
                        let again = panel.refresh_pending && !panel.index_busy;
                        if again {
                            panel.refresh_pending = false;
                        } else {
                            panel.refreshing = false;
                        }
                        cx.notify();
                        again
                    })
                });
                if !matches!(again, Ok(true)) {
                    return;
                }
            }
        }));
        cx.notify();
    }
}
