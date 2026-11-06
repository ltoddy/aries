use agent_client_protocol::v1::schema::{SessionUpdate, ToolCallContent};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::chat::model::tool_call::ToolCallState;
use crate::chat::view::ChatView;
use crate::chat::view::palette::Palette;
use crate::chat::view::timeline::markdown_style;

impl ChatView {
    pub fn cache_tool_diffs(&mut self, index: usize, cx: &mut Context<Self>) {
        let message = &self.chat.messages()[index];
        let Some(_) = message.tool_call() else {
            return;
        };
        if matches!(message.updates().last().map(|update| &update.update), Some(SessionUpdate::ToolCallUpdate(update)) if update.fields.content.is_none())
        {
            return;
        }
        self.build_tool_diffs(index, cx);
    }

    pub fn rebuild_tool_diffs(&mut self, cx: &mut Context<Self>) {
        self.tool_diffs.clear();
        for index in 0..self.chat.messages().len() {
            self.build_tool_diffs(index, cx);
        }
    }

    fn build_tool_diffs(&mut self, index: usize, _cx: &mut Context<Self>) {
        let Some(tool) = self.chat.messages()[index].tool_call() else {
            return;
        };
        self.tool_diffs.retain(|(message, _), _| *message != index);
        for (content_index, content) in tool.call().content.iter().enumerate() {
            if let ToolCallContent::Diff(diff) = content {
                let old = diff.old_text.as_deref().unwrap_or_default();
                let new = &diff.new_text;
                let path = diff.path.to_string_lossy().replace(['\n', '\r', '\t'], " ");
                let patch = similar::TextDiff::configure()
                    .timeout(std::time::Duration::from_millis(100))
                    .diff_lines(old, new)
                    .unified_diff()
                    .context_radius(3)
                    .header(if diff.old_text.is_some() { &path } else { "/dev/null" }, &path)
                    .to_string();
                if !patch.is_empty() {
                    let markdown =
                        format!("`````diff\ndiff --git a/{path} b/{path}\n{patch}\n`````");
                    self.tool_diffs.insert((index, content_index), markdown);
                }
            }
        }
    }

    pub fn render_tool_content(
        &self,
        index: usize,
        tool: &ToolCallState,
        p: Palette,
        _window: &Window,
        _cx: &App,
    ) -> AnyElement {
        let call = tool.call();
        div()
            .id(("tool-details", index))
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .items_start()
            .gap_1()
            .text_xs()
            .text_color(p.muted())
            .children(call.content.iter().enumerate().filter_map(|(content_index, content)| {
                let ToolCallContent::Diff(diff) = content else {
                    return None;
                };
                let id = SharedString::from(format!("tool-content:{index}:{content_index}"));
                Some(match self.tool_diffs.get(&(index, content_index)) {
                    Some(markdown) => {
                        let markdown = markdown.clone();
                        TextView::markdown(id, markdown)
                            .w_full()
                            .min_w_0()
                            .style(markdown_style())
                            .selectable(true)
                            .scrollable(false)
                            .code_block_actions(|code_block, _window, cx| {
                                let code = code_block.code().to_string();
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .when_some(code_block.lang(), |el, lang| {
                                        el.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().muted)
                                                .child(lang.to_string()),
                                        )
                                    })
                                    .child({
                                        Button::new("copy-code-block")
                                            .ghost()
                                            .xsmall()
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    code.clone(),
                                                ));
                                            })
                                            .child(
                                                Icon::new(IconName::Copy)
                                                    .xsmall()
                                                    .text_color(cx.theme().muted),
                                            )
                                    })
                                    .into_any_element()
                            })
                            .text_sm()
                            .into_any_element()
                    }
                    None => div()
                        .w_full()
                        .child(diff.path.to_string_lossy().into_owned())
                        .into_any_element(),
                })
            }))
            .into_any_element()
    }
}
