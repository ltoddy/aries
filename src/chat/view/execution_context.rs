use std::time::Duration;

use gpui_kit::assets::IconName;
use gpui_kit::base::motion::{self, Transition};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::git_info::GitInfo;
use crate::chat::view::palette::Palette;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionState {
    Disconnected,
    Deleting,
    Stopping,
    Permission,
    Connecting,
    ChoosingProject,
    Loading,
    Creating,
    Configuring,
    Running,
    Failed,
    Idle,
}

impl ExecutionState {
    pub fn render_tab_indicator(self, p: Palette) -> AnyElement {
        let indicator = match self {
            Self::Idle => None,
            Self::Disconnected | Self::Failed => Some(
                Icon::new(IconName::CircleX).size_3().text_color(p.danger()).into_any_element(),
            ),
            Self::Permission => Some(
                Icon::new(IconName::CircleQuestionMark)
                    .size_3()
                    .text_color(p.foreground())
                    .into_any_element(),
            ),
            Self::ChoosingProject => {
                Some(Icon::new(IconName::Folder).size_3().text_color(p.muted()).into_any_element())
            }
            _ => Some(Spinner::new().small().color(p.muted()).into_any_element()),
        };
        div()
            .size_4()
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_center()
            .children(indicator)
            .into_any_element()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Disconnected => "连接已断开，请新建会话",
            Self::Deleting => "删除会话中",
            Self::Stopping => "正在停止",
            Self::Permission => "等待授权",
            Self::Connecting => "连接中",
            Self::ChoosingProject => "选择项目中",
            Self::Loading => "加载会话中",
            Self::Creating => "创建会话中",
            Self::Configuring => "更新配置中",
            Self::Running => "执行中",
            Self::Failed => "执行失败",
            Self::Idle => "空闲",
        }
    }
}

impl ChatView {
    pub fn watch_git_info(&mut self, cx: &mut Context<Self>) {
        if self.git_motion.reset(&self.cwd) {
            self.git_info = None;
            self.git_read_error = None;
            self.git_info_collapsed = true;
        }
        self.git_revision += 1;
        self.git_refresh_task = None;
        self.git_refreshing = false;
        self.git_refresh_pending = false;
        self.git_watch_error = None;
        let directory = self.cwd.clone();
        self.git_info_task = Some(cx.spawn(async move |view, cx| {
            let (sender, receiver) = async_channel::bounded(1);
            let executor = cx.background_executor().clone();
            let watcher = cx
                .background_spawn(async move {
                    crate::chat::view::git_info::watch::GitWatch::start(
                        &directory, sender, executor,
                    )
                })
                .await;
            let watcher = match watcher {
                Ok(watcher) => watcher,
                Err(error) => {
                    let _ = view.update(cx, |this, cx| {
                        this.git_watch_error = Some(error);
                        this.refresh_git_info(cx);
                    });
                    return;
                }
            };
            if view.update(cx, |this, cx| this.refresh_git_info(cx)).is_err() {
                return;
            }
            while receiver.recv().await.is_ok() {
                cx.background_executor().timer(Duration::from_millis(300)).await;
                while receiver.try_recv().is_ok() {}
                let error = watcher.take_error();
                if view
                    .update(cx, |this, cx| {
                        if let Some(error) = error {
                            this.git_watch_error = Some(error);
                        }
                        this.refresh_git_info(cx);
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

    pub fn refresh_git_info(&mut self, cx: &mut Context<Self>) {
        if self.git_refreshing || self.git_index_busy {
            self.git_refresh_pending = true;
            return;
        }
        self.git_refreshing = true;
        self.git_refresh_pending = false;
        self.git_refresh_task = Some(cx.spawn(async move |view, cx| {
            loop {
                let Ok((directory, revision)) =
                    view.read_with(cx, |this, _| (this.cwd.clone(), this.git_revision))
                else {
                    return;
                };
                let (info, directory) = cx
                    .background_spawn(async move { (GitInfo::read(&directory), directory) })
                    .await;
                let again = view.update(cx, |this, cx| {
                    if crate::chat::view::git_info::motion::accept_result(
                        &this.cwd,
                        this.git_revision,
                        &directory,
                        revision,
                    ) && !this.git_index_busy
                    {
                        this.sync_git_info(info, true, cx);
                    }
                    let again = this.git_refresh_pending && !this.git_index_busy;
                    if again {
                        this.git_refresh_pending = false;
                    } else {
                        this.git_refreshing = false;
                    }
                    cx.notify();
                    again
                });
                if !matches!(again, Ok(true)) {
                    return;
                }
            }
        }));
        cx.notify();
    }

    pub fn execution_state(&self) -> ExecutionState {
        execution_state([
            (self.disconnected, ExecutionState::Disconnected),
            (self.stopping, ExecutionState::Stopping),
            (self.session_deletion.is_busy(), ExecutionState::Deleting),
            (!self.permissions.is_empty(), ExecutionState::Permission),
            (self.session_list.is_connecting(), ExecutionState::Connecting),
            (self.choosing_directory, ExecutionState::ChoosingProject),
            (
                self.loading_session.is_some() || self.queued_session_load.is_some(),
                ExecutionState::Loading,
            ),
            (self.creating_session, ExecutionState::Creating),
            (self.pending_config.is_some(), ExecutionState::Configuring),
            (self.chat.is_busy(), ExecutionState::Running),
            (self.chat.error().is_some(), ExecutionState::Failed),
        ])
    }

    pub fn render_execution_context(&self, p: Palette, cx: &Context<Self>) -> impl IntoElement {
        let selected = self.session_id.is_some() || self.session_directory_selected;
        let project = if selected {
            self.cwd
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.cwd.display().to_string())
        } else {
            "新建会话时选择项目目录".into()
        };
        let mut directory = if selected {
            format!("执行目录：{}", self.cwd.display())
        } else {
            "请通过左侧新建会话按钮或 ⌘N 选择项目目录".into()
        };
        let branch = selected
            .then_some(self.git_info.as_ref())
            .flatten()
            .map(|info| info.branch().to_owned());
        if let Some(branch) = &branch {
            directory.push_str(&format!("\nGit：{branch}"));
        }
        div()
            .flex()
            .items_center()
            .gap_3()
            .min_w_0()
            .min_h_6()
            .text_xs()
            .text_color(p.muted())
            .child(
                div()
                    .id("execution-project")
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .aria_label(directory.clone())
                    .tooltip(move |window, cx| {
                        let directory = directory.clone();
                        Tooltip::element(move |_, _| {
                            div().max_w(rems(36.)).whitespace_normal().child(directory.clone())
                        })
                        .build(window, cx)
                    })
                    .child(Icon::new(IconName::Folder).size_3().flex_shrink_0())
                    .child(div().min_w_0().truncate().child(project))
                    .when_some(branch, |row, branch| {
                        row.child(
                            div()
                                .min_w_0()
                                .max_w(relative(0.5))
                                .truncate()
                                .child(format!("· {branch}")),
                        )
                    }),
            )
            .when_some(self.tokens, |row, tokens| {
                row.child(TokenUsage {
                    id: ElementId::NamedChild(
                        ElementId::from(("usage", cx.entity_id())).into(),
                        format!("usage-{:?}", self.session_id).into(),
                    ),
                    tokens,
                })
            })
    }
}

#[derive(IntoElement)]
struct TokenUsage {
    id: ElementId,
    tokens: u64,
}

impl RenderOnce for TokenUsage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let digits = self.tokens.to_string();
        let duration = cx.theme().motion_tokens().duration_normal;
        let mut number = div().flex().font_family(cx.theme().mono_font_family.clone());
        for (ix, digit) in digits.bytes().enumerate() {
            let place = digits.len() - ix - 1;
            let position = motion::transition(
                (self.id.clone(), SharedString::from(format!("digit-{place}"))),
                f32::from(digit - b'0'),
                Transition::new(duration),
                window,
                cx,
            );
            let lower = position.floor();
            let fraction = position - lower;
            number = number.child(div().relative().w(rems(0.5)).h_4().overflow_hidden().children(
                (0..=1).filter_map(|offset| {
                    let value = lower as u8 + offset;
                    (value <= 9).then(|| {
                        div()
                            .absolute()
                            .top(rems(f32::from(offset) - fraction))
                            .left_0()
                            .w_full()
                            .h_4()
                            .line_height(rems(1.))
                            .text_center()
                            .child(value.to_string())
                    })
                }),
            ));
        }
        div()
            .id(self.id)
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_1()
            .aria_label(format!("{} tokens", self.tokens))
            .child(number)
            .child("tokens")
    }
}

fn execution_state(states: impl IntoIterator<Item = (bool, ExecutionState)>) -> ExecutionState {
    states
        .into_iter()
        .find_map(|(active, state)| active.then_some(state))
        .unwrap_or(ExecutionState::Idle)
}
