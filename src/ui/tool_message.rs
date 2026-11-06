use agent_client_protocol::v1::schema::{ToolCallStatus, ToolKind};
use gpui_kit::assets::IconName;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::session::tool_call::ToolCallState;
use crate::ui::ChatView;
use crate::ui::palette::Palette;

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

impl ChatView {
    pub fn render_tool_message(
        &self,
        index: usize,
        tool: &ToolCallState,
        p: Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
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
        let title = crate::ui::single_line(tool.title());
        let details = format!("{} · {status}", tool.title());
        div()
            .id(("message", index))
            .aria_label(format!("工具调用：{} · {status}", tool.title()))
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
                                    .child(
                                        if tool.call().status == ToolCallStatus::InProgress
                                            && !tool.is_interrupted()
                                        {
                                            Spinner::new().small().color(color).into_any_element()
                                        } else {
                                            Icon::new(status_icon).size(px(14.)).into_any_element()
                                        },
                                    )
                                    .when(failed || tool.is_interrupted(), |status_row| {
                                        status_row.child(status)
                                    }),
                            )
                            .child(
                                Icon::new(tool_icon(tool.call().kind))
                                    .size(px(14.))
                                    .flex_shrink_0(),
                            )
                            .child(div().flex_1().min_w_0().child(
                                div().w_full().min_w_0().text_left().truncate().child(title),
                            )),
                    ),
            )
            .child(self.render_tool_content(index, tool, p, window, cx))
            .into_any_element()
    }
}
