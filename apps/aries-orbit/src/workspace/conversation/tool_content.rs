use std::collections::HashMap;

use agent_client_protocol::v1::schema::{SessionUpdate, ToolCallContent};
use gpui_kit::assets::IconName;
use gpui_kit::base::SelectableText;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::conversation::model::Conversation;
use crate::workspace::conversation::model::tool_call::ToolCallState;
use crate::workspace::conversation::timeline::markdown_style;
use crate::workspace::settings::palette::Palette;

#[derive(IntoElement)]
struct ToolContent {
    index: usize,
    entries: Vec<ToolDiff>,
    palette: Palette,
}

impl ToolContent {
    fn new(index: usize, entries: Vec<ToolDiff>, palette: Palette) -> Self {
        Self { index, entries, palette }
    }
}

impl RenderOnce for ToolContent {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self { index, entries, palette: p } = self;
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
            .children(entries)
    }
}

enum DiffContent {
    Markdown(String),
    Path(String),
}

#[derive(IntoElement)]
struct ToolDiff {
    id: SharedString,
    content: DiffContent,
}

impl ToolDiff {
    fn new(id: SharedString, content: DiffContent) -> Self {
        Self { id, content }
    }
}

impl RenderOnce for ToolDiff {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let Self { id, content } = self;
        match content {
            DiffContent::Markdown(markdown) => TextView::markdown(id, markdown)
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
                                    cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                                })
                                .child(
                                    Icon::new(IconName::Copy).xsmall().text_color(cx.theme().muted),
                                )
                        })
                        .into_any_element()
                })
                .text_sm()
                .into_any_element(),
            DiffContent::Path(path) => {
                div().w_full().child(SelectableText::new(id, path)).into_any_element()
            },
        }
    }
}

pub struct ToolDiffs {
    patches: HashMap<(usize, usize), String>,
}
impl ToolDiffs {
    pub fn new() -> Self {
        Self { patches: HashMap::new() }
    }
    pub fn clear(&mut self) {
        self.patches.clear();
    }

    pub fn cache_tool_diffs(&mut self, index: usize, conversation: &Conversation) {
        let message = &conversation.messages()[index];
        let Some(_) = message.tool_call() else {
            return;
        };
        if matches!(message.updates().last().map(|update| &update.update), Some(SessionUpdate::ToolCallUpdate(update)) if update.fields.content.is_none())
        {
            return;
        }
        self.build_tool_diffs(index, conversation);
    }

    pub fn rebuild_tool_diffs(&mut self, conversation: &Conversation) {
        self.patches.clear();
        for index in 0..conversation.messages().len() {
            self.build_tool_diffs(index, conversation);
        }
    }

    fn build_tool_diffs(&mut self, index: usize, conversation: &Conversation) {
        let Some(tool) = conversation.messages()[index].tool_call() else {
            return;
        };
        self.patches.retain(|(message, _), _| *message != index);
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
                    self.patches.insert((index, content_index), markdown);
                }
            }
        }
    }

    pub fn render(&self, index: usize, tool: &ToolCallState, p: Palette) -> AnyElement {
        let entries = tool
            .call()
            .content
            .iter()
            .enumerate()
            .filter_map(|(content_index, content)| {
                let ToolCallContent::Diff(diff) = content else {
                    return None;
                };
                Some(ToolDiff::new(
                    SharedString::from(format!("tool-content:{index}:{content_index}")),
                    self.patches
                        .get(&(index, content_index))
                        .cloned()
                        .map(DiffContent::Markdown)
                        .unwrap_or_else(|| {
                            DiffContent::Path(diff.path.to_string_lossy().into_owned())
                        }),
                ))
            })
            .collect();
        ToolContent::new(index, entries, p).into_any_element()
    }
}
