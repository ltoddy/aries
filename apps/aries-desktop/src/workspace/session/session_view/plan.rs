use agent_client_protocol::v1::schema::{PlanEntry, PlanEntryStatus};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::progress::Progress;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::{Icon, Sizable};

use crate::workspace::session::session_view::*;

impl SessionView {
    pub fn render_plan(&self, p: Palette, cx: &mut Context<Self>) -> AnyElement {
        let plan = self.execution.conversation().plan();
        if plan.is_empty() {
            return div().into_any_element();
        }
        let total = plan.len();
        let done = plan.iter().filter(|entry| entry.status == PlanEntryStatus::Completed).count();
        let active = plan.iter().find(|entry| entry.status == PlanEntryStatus::InProgress);
        let collapsed = self.plan_collapsed;
        let primary = cx.theme().primary;
        let success = cx.theme().success;
        let radius = cx.theme().radius_lg;
        let row_radius = cx.theme().radius;
        let header = div()
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .child(
                Button::new(("session-plan-toggle", cx.entity_id()))
                    .small()
                    .ghost()
                    .icon(if collapsed { IconName::ChevronRight } else { IconName::ChevronDown })
                    .label("计划")
                    .aria_expanded(!collapsed)
                    .accessibility_label(if collapsed { "展开计划" } else { "收起计划" })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.plan_collapsed = !this.plan_collapsed;
                        cx.notify();
                    })),
            )
            .child(div().min_w_0().flex_1().truncate().text_xs().text_color(p.muted()).when(
                collapsed,
                |summary| {
                    summary.when_some(active, |summary, entry| summary.child(entry_text(entry)))
                },
            ))
            .child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(p.muted())
                    .child(format!("{done}/{total} 已完成")),
            )
            .child(
                Progress::new(("session-plan-progress", cx.entity_id()))
                    .xsmall()
                    .w_16()
                    .flex_shrink_0()
                    .color(if done == total { success } else { primary })
                    .value(done as f32 / total as f32 * 100.)
                    .accessibility_label("计划完成进度"),
            );
        let mut panel = div()
            .id("session-plan")
            .w_full()
            .min_w_0()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .rounded(radius)
            .border_1()
            .border_color(p.border().opacity(0.5))
            .bg(cx.theme().group_box)
            .child(header);
        if !collapsed {
            let mut entries = div().flex().flex_col().gap_1().px_2().pb_2();
            for entry in plan {
                let in_progress = entry.status == PlanEntryStatus::InProgress;
                let (icon, color) = match entry.status {
                    PlanEntryStatus::Completed => (IconName::CircleCheck, success),
                    PlanEntryStatus::InProgress => (IconName::Loader, primary),
                    _ => (IconName::Circle, p.muted()),
                };
                entries = entries.child(
                    div()
                        .flex()
                        .items_start()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded(row_radius)
                        .when(in_progress, |row| row.bg(primary.opacity(0.06)))
                        .child(
                            div()
                                .size_5()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(Icon::new(icon).size(rems(0.875)).text_color(color)),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_sm()
                                .line_height(relative(1.5))
                                .text_color(p.muted())
                                .when(in_progress, |text| {
                                    text.text_color(p.foreground()).font_weight(FontWeight::MEDIUM)
                                })
                                .child(entry_text(entry)),
                        )
                        .when(in_progress, |row| {
                            row.child(
                                div().flex_shrink_0().text_xs().text_color(primary).child("进行中"),
                            )
                        }),
                );
            }
            panel = panel.child(
                div()
                    .id("session-plan-entries")
                    .min_h_0()
                    .max_h(rems(12.0))
                    .pb_1()
                    .rounded_b(radius)
                    .overflow_y_scrollbar()
                    .child(entries),
            );
        }
        panel.into_any_element()
    }
}

fn entry_text(entry: &PlanEntry) -> String {
    if entry.status == PlanEntryStatus::InProgress
        && let Some(form) = entry
            .meta
            .as_ref()
            .and_then(|meta| meta.get("activeForm"))
            .and_then(|value| value.as_str())
    {
        return form.to_owned();
    }
    entry.content.clone()
}
