use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::dialog::DialogButtonProps;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::conversation::model::display::code_block;
use crate::workspace::conversation::single_line;
use crate::workspace::settings::typography;

fn pretty(value: &serde_json::Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

impl SessionView {
    pub fn open_tool_details(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tool) = self.with_timeline(|_, conversation, _| {
            conversation.messages().get(index).and_then(|message| message.tool_call().cloned())
        }) else {
            return;
        };
        let call = tool.call();
        let id = SharedString::from(call.tool_call_id.to_string());
        let title = single_line(tool.title());
        let input = call.raw_input.as_ref().map(pretty);
        let output = call.raw_output.as_ref().map(pretty);
        window.open_dialog(cx, move |dialog, _, cx| {
            let input = input.clone();
            let output = output.clone();
            let id = id.clone();
            dialog
                .title(div().min_w_0().pr_6().truncate().child(title.clone()))
                .centered()
                .width(cx.theme().font_size * 68.)
                .button_props(DialogButtonProps::default().ok_text("关闭"))
                .content(move |content, window, cx| {
                    let body = id.clone();
                    content.child(
                        div()
                            .id(SharedString::from(format!("tool-details:{body}")))
                            .max_h(
                                (window.viewport_size().height * 0.72)
                                    .min(cx.theme().font_size * 42.),
                            )
                            .overflow_y_scroll()
                            .child(ToolDetails::new(body, input.clone(), output.clone())),
                    )
                })
        });
    }
}

#[derive(IntoElement)]
struct ToolDetails {
    id: SharedString,
    input: Option<String>,
    output: Option<String>,
}

impl ToolDetails {
    fn new(id: SharedString, input: Option<String>, output: Option<String>) -> Self {
        Self { id, input, output }
    }
}

impl RenderOnce for ToolDetails {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { id, input, output } = self;
        let sections = [("input", "原始输入", input), ("output", "原始输出", output)];
        let entries = sections
            .into_iter()
            .filter_map(|(suffix, label, text)| {
                let text = text?;
                Some(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(label),
                                )
                                .child(
                                    Clipboard::new(SharedString::from(format!(
                                        "{id}:copy:{suffix}"
                                    )))
                                    .value(SharedString::from(text.clone()))
                                    .accessibility_label(format!("复制{label}")),
                                ),
                        )
                        .child(
                            TextView::markdown(
                                SharedString::from(format!("{id}:{suffix}")),
                                code_block("json", &text),
                            )
                            .w_full()
                            .min_w_0()
                            .map(typography::style_markdown)
                            .selectable(true)
                            .scrollable(false),
                        ),
                )
            })
            .collect::<Vec<_>>();
        div()
            .w_full()
            .min_w_0()
            .flex()
            .items_start()
            .gap_4()
            .when(entries.is_empty(), |body| {
                body.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("没有原始输入或输出"),
                )
            })
            .children(entries)
    }
}
