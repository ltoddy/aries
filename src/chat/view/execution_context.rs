use std::path::Path;
use std::time::Duration;

use git2::Repository;
use gpui_kit::assets::IconName;
use gpui_kit::component::Icon;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum GitBranch {
    #[default]
    Unavailable,
    Named(String),
    Detached(String),
}

impl GitBranch {
    fn read(cwd: &Path) -> Self {
        let Ok(repository) = Repository::discover(cwd) else {
            return Self::Unavailable;
        };
        let Ok(head) = repository.find_reference("HEAD") else {
            return Self::Unavailable;
        };
        let Ok(target) = head.symbolic_target() else {
            return Self::Unavailable;
        };
        if let Some(branch) = target.and_then(|name| name.strip_prefix("refs/heads/")) {
            return Self::Named(branch.to_owned());
        }
        head.target()
            .map(|commit| Self::Detached(commit.to_string()[..7].to_owned()))
            .unwrap_or_default()
    }

    fn label(&self) -> Option<String> {
        match self {
            Self::Unavailable => None,
            Self::Named(branch) => Some(branch.clone()),
            Self::Detached(commit) => Some(format!("游离 HEAD · {commit}")),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExecutionState {
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
    fn label(self) -> &'static str {
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
    pub(super) fn watch_git_branch(&mut self, cx: &mut Context<Self>) {
        self.git_branch = GitBranch::Unavailable;
        self.git_branch_task = Some(cx.spawn(async move |view, cx| {
            loop {
                let Ok(directory) = view.read_with(cx, |this, _| this.cwd.clone()) else {
                    return;
                };
                let cwd = directory.clone();
                let branch = cx.background_spawn(async move { GitBranch::read(&cwd) }).await;
                if view
                    .update(cx, |this, cx| {
                        if this.cwd == directory && this.git_branch != branch {
                            this.git_branch = branch;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    return;
                }
                cx.background_executor().timer(Duration::from_secs(2)).await;
            }
        }));
    }

    fn execution_state(&self) -> ExecutionState {
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
            (self.creating_session.is_some(), ExecutionState::Creating),
            (self.pending_config.is_some(), ExecutionState::Configuring),
            (self.chat.is_busy(), ExecutionState::Running),
            (self.chat.error().is_some(), ExecutionState::Failed),
        ])
    }

    pub(super) fn render_execution_context(&self, p: Palette) -> impl IntoElement {
        let selected = self.session_id.is_some() || self.session_directory_selected;
        let project = if selected {
            self.cwd
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.cwd.display().to_string())
        } else {
            "发送时选择项目目录".into()
        };
        let mut directory = if selected {
            format!("执行目录：{}", self.cwd.display())
        } else {
            "发送前需选择项目目录".into()
        };
        let branch = selected.then(|| self.git_branch.label()).flatten();
        if let Some(branch) = &branch {
            directory.push_str(&format!("\nGit：{branch}"));
        }
        let state = self.execution_state();
        let label =
            if state == ExecutionState::Failed { self.status.as_str() } else { state.label() };
        let detail = if matches!(state, ExecutionState::Disconnected | ExecutionState::Failed) {
            self.chat.error().unwrap_or("连接已断开").to_owned()
        } else if self.chat.is_busy() {
            self.status.clone()
        } else {
            state.label().into()
        };
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
            .child(
                div()
                    .id("execution-state")
                    .min_w_0()
                    .max_w(relative(0.5))
                    .truncate()
                    .text_color(match state {
                        ExecutionState::Disconnected | ExecutionState::Failed => p.danger(),
                        ExecutionState::Permission => p.foreground(),
                        _ => p.muted(),
                    })
                    .aria_label(format!("执行状态：{label}"))
                    .tooltip(move |window, cx| {
                        let detail = detail.clone();
                        Tooltip::element(move |_, _| {
                            div().max_w(rems(30.)).whitespace_normal().child(detail.clone())
                        })
                        .build(window, cx)
                    })
                    .child(label.to_owned()),
            )
    }
}

fn execution_state(states: impl IntoIterator<Item = (bool, ExecutionState)>) -> ExecutionState {
    states
        .into_iter()
        .find_map(|(active, state)| active.then_some(state))
        .unwrap_or(ExecutionState::Idle)
}
