use gpui_kit::assets::IconName;
use gpui_kit::base::SelectableText;
use gpui_kit::base::motion::{self, Transition};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::settings::palette::Palette;
use crate::workspace::{SessionView, number};

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
            },
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

pub fn render_execution_context(
    owner: &SessionView,
    p: Palette,
    cx: &Context<SessionView>,
) -> impl IntoElement {
    let selected =
        owner.execution().session_id().is_some() || owner.execution().directory_selected();
    let project = if selected {
        owner
            .execution()
            .directory()
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| owner.execution().directory().display().to_string())
    } else {
        "新建会话时选择项目目录".into()
    };
    let mut directory = if selected {
        format!("执行目录：{}", owner.execution().directory().display())
    } else {
        "通过左侧“新建会话…”按钮选择项目目录，或按 ⌘N 使用当前目录新建会话".into()
    };
    let branch =
        selected.then_some(owner.git_panel().info()).flatten().map(|info| info.branch().to_owned());
    if let Some(branch) = &branch {
        directory.push_str(&format!("\nGit：{branch}"));
    }
    if let Some(id) = owner.execution().session_id() {
        directory.push_str(&format!("\nSession ID: {id}"));
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
                .child(
                    div()
                        .min_w_0()
                        .truncate()
                        .child(SelectableText::new("execution-project-text", project)),
                )
                .when_some(branch, |row, branch| {
                    row.child(
                        div().min_w_0().max_w(relative(0.5)).truncate().child(SelectableText::new(
                            "execution-branch-text",
                            format!("· {branch}"),
                        )),
                    )
                })
                .when_some(owner.execution().session_id(), |row, id| {
                    row.child(
                        div()
                            .min_w_0()
                            .max_w(relative(0.5))
                            .truncate()
                            .font_family(cx.theme().mono_font_family.clone())
                            .child(SelectableText::new(
                                "execution-session-text",
                                format!("· Session ID: {id}"),
                            )),
                    )
                }),
        )
        .when_some(owner.execution().tokens(), |row, tokens| {
            row.child(TokenUsage {
                id: ElementId::NamedChild(
                    ElementId::from(("usage", cx.entity_id())).into(),
                    match owner.execution().session_id() {
                        Some(id) => format!("session:{id}").into(),
                        None => "no-session".into(),
                    },
                ),
                tokens,
            })
        })
}

#[derive(IntoElement)]
struct TokenUsage {
    id: ElementId,
    tokens: u64,
}

impl RenderOnce for TokenUsage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let digits = number::compact(self.tokens);
        let duration = cx.theme().motion_tokens().duration_normal;
        let mut number = div().flex().font_family(cx.theme().mono_font_family.clone());
        for (ix, digit) in digits.bytes().enumerate() {
            if !digit.is_ascii_digit() {
                number = number
                    .child(div().h_4().line_height(rems(1.)).child(char::from(digit).to_string()));
                continue;
            }
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
            .aria_label(format!("{} tokens", digits))
            .child(number)
            .child("tokens")
    }
}
