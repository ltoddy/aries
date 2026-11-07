use agent_client_protocol::v1::schema::{SessionId, ToolCallStatus, ToolKind};
use gpui_kit::assets::IconName;
use gpui_kit::base::motion::{self, Transition};
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::conversation::model::tool_call::ToolCallState;
use crate::workspace::settings::palette::Palette;

fn tool_icon(kind: ToolKind) -> IconName {
    match kind {
        ToolKind::Read => IconName::Eye,
        ToolKind::Edit => IconName::FilePenLine,
        ToolKind::Delete => IconName::Trash,
        ToolKind::Move => IconName::FileSymlink,
        ToolKind::Search => IconName::Search,
        ToolKind::Execute => IconName::SquareTerminal,
        ToolKind::Think => IconName::Brain,
        ToolKind::Fetch => IconName::Globe,
        ToolKind::SwitchMode => IconName::Settings,
        _ => IconName::Wrench,
    }
}

#[derive(IntoElement)]
pub struct ToolMessage {
    index: usize,
    header: ToolHeader,
    content: AnyElement,
    palette: Palette,
}

impl ToolMessage {
    pub fn from_tool(
        index: usize,
        tool: &ToolCallState,
        p: Palette,
        entity_id: EntityId,
        session_id: Option<&SessionId>,
        content: AnyElement,
    ) -> Self {
        let failed = tool.call().status == ToolCallStatus::Failed;
        let (status, status_icon) = if tool.is_interrupted() {
            ("已中断", IconName::CirclePause)
        } else {
            match tool.call().status {
                ToolCallStatus::Pending => ("等待中", IconName::Clock),
                ToolCallStatus::InProgress => ("进行中", IconName::Loader),
                ToolCallStatus::Completed => ("已完成", IconName::CircleCheck),
                ToolCallStatus::Failed => ("失败", IconName::CircleX),
                _ => ("未知状态", IconName::CircleQuestionMark),
            }
        };
        let color = if failed { p.danger() } else { p.muted() };
        let title = crate::workspace::conversation::single_line(tool.title());
        let details = SharedString::from(format!("{} · {status}", tool.title()));
        let status_icon = MessageStatusIcon {
            id: ElementId::NamedChild(
                ElementId::from(("tool-status-motion", entity_id)).into(),
                format!("{:?}-{}", session_id, tool.call().tool_call_id).into(),
            ),
            running: tool.call().status == ToolCallStatus::InProgress && !tool.is_interrupted(),
            completed: tool.call().status == ToolCallStatus::Completed && !tool.is_interrupted(),
            icon: status_icon,
            color,
        };
        ToolMessage::new(
            index,
            ToolHeader::new(
                format!("工具调用：{} · {status}", tool.title()),
                title,
                details,
                tool_icon(tool.call().kind),
                ToolStatus::new(status, failed || tool.is_interrupted(), status_icon),
            ),
            content,
            p,
        )
    }

    fn new(index: usize, header: ToolHeader, content: AnyElement, palette: Palette) -> Self {
        Self { index, header, content, palette }
    }
}

struct ToolHeader {
    label: String,
    title: String,
    details: SharedString,
    icon: IconName,
    status: ToolStatus,
}

impl ToolHeader {
    fn new(
        label: String,
        title: String,
        details: SharedString,
        icon: IconName,
        status: ToolStatus,
    ) -> Self {
        Self { label, title, details, icon, status }
    }
}

struct ToolStatus {
    label: &'static str,
    visible: bool,
    icon: MessageStatusIcon,
}

impl ToolStatus {
    fn new(label: &'static str, visible: bool, icon: MessageStatusIcon) -> Self {
        Self { label, visible, icon }
    }
}

impl RenderOnce for ToolMessage {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self { index, header, content, palette: p } = self;
        let ToolHeader { label, title, details, icon, status } = header;
        let ToolStatus { label: status, visible: show_status, icon: status_icon } = status;
        let color = status_icon.color;
        div()
            .id(("message", index))
            .aria_label(label)
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .id(("tool-header", index))
                    .w_full()
                    .min_w_0()
                    .tooltip(move |window, cx| {
                        let details = details.clone();
                        Tooltip::element(move |_, _| {
                            div().max_w(px(480.)).whitespace_normal().child(details.clone())
                        })
                        .build(window, cx)
                    })
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .h_6()
                            .flex()
                            .items_center()
                            .justify_start()
                            .gap_2()
                            .text_xs()
                            .text_color(p.muted())
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_color(color)
                                    .child(status_icon)
                                    .when(show_status, |status_row| status_row.child(status)),
                            )
                            .child(Icon::new(icon).size(px(14.)).flex_shrink_0())
                            .child(div().flex_1().min_w_0().child(
                                div().w_full().min_w_0().text_left().truncate().child(title),
                            )),
                    ),
            )
            .child(content)
            .into_any_element()
    }
}

#[derive(IntoElement)]
pub struct MessageStatusIcon {
    id: ElementId,
    running: bool,
    completed: bool,
    icon: IconName,
    color: Hsla,
}

impl MessageStatusIcon {
    pub fn new(id: ElementId, running: bool, completed: bool, icon: IconName, color: Hsla) -> Self {
        Self { id, running, completed, icon, color }
    }
}

impl RenderOnce for MessageStatusIcon {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion = cx.theme().motion_tokens();
        let completion = motion::transition(
            (self.id, "completion"),
            if self.completed { 1. } else { 0. },
            Transition::new(motion.duration_fast).easing(motion.easing_enter.clone()),
            window,
            cx,
        );
        div().size(rems(0.875)).flex().items_center().justify_center().child(if self.running {
            Spinner::new().small().color(self.color).into_any_element()
        } else {
            Icon::new(self.icon)
                .size(rems(0.875))
                .text_color(self.color)
                .opacity(if self.completed { completion } else { 1. })
                .into_any_element()
        })
    }
}
