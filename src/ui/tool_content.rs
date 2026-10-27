use std::borrow::Cow;

use agent_client_protocol::schema::v1::{SessionUpdate, ToolCallContent};
use gpui_kit::component::ActiveTheme;
use gpui_kit::component::diff::{Diff, DiffFile, DiffState};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::session::tool_call::ToolCallState;
use crate::ui::ChatView;
use crate::ui::palette::Palette;

fn with_final_newline(text: &str) -> Cow<'_, str> {
    if !text.is_empty() && !text.ends_with('\n') {
        Cow::Owned(format!("{text}\n"))
    } else {
        Cow::Borrowed(text)
    }
}

fn diff_shape(patch: &str) -> (usize, usize) {
    let mut rows = 0;
    let mut hunks = 0;
    for line in patch.lines() {
        if line.starts_with("@@") {
            hunks += 1;
        } else if line.starts_with(' ')
            || (line.starts_with('+') && !line.starts_with("+++"))
            || (line.starts_with('-') && !line.starts_with("---"))
        {
            rows += 1;
        }
    }
    (rows, hunks)
}

fn diff_height(rows: usize, hunks: usize, window: &Window, cx: &App) -> Pixels {
    let rem = window.rem_size();
    let font_size = rem * (f32::from(cx.theme().mono_font_size) / 16.);
    let code_row = font_size * 1.6;
    let section = rem * 1.5;
    rem * 2. + section * (1. + hunks as f32) + code_row * rows as f32
}

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
        self.tool_diff_rows.clear();
        for index in 0..self.chat.messages().len() {
            self.build_tool_diffs(index, cx);
        }
    }

    fn build_tool_diffs(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(tool) = self.chat.messages()[index].tool_call() else {
            return;
        };
        self.tool_diffs.retain(|(message, _), _| *message != index);
        self.tool_diff_rows.retain(|(message, _), _| *message != index);
        for (content_index, content) in tool.call().content.iter().enumerate() {
            if let ToolCallContent::Diff(diff) = content {
                let old = with_final_newline(diff.old_text.as_deref().unwrap_or_default());
                let new = with_final_newline(&diff.new_text);
                let path = diff.path.to_string_lossy().replace(['\n', '\r', '\t'], " ");
                let patch = similar::TextDiff::configure()
                    .timeout(std::time::Duration::from_millis(100))
                    .diff_lines(&old, &new)
                    .unified_diff()
                    .context_radius(3)
                    .header(if diff.old_text.is_some() { &path } else { "/dev/null" }, &path)
                    .to_string();
                if let Ok(files) = DiffFile::parse(&patch)
                    && !files.is_empty()
                {
                    self.tool_diff_rows.insert((index, content_index), diff_shape(&patch));
                    self.tool_diffs.insert(
                        (index, content_index),
                        cx.new(|cx| DiffState::new(files, cx).with_context_lines(None)),
                    );
                }
            }
        }
    }

    pub fn render_tool_content(
        &self,
        index: usize,
        tool: &ToolCallState,
        p: Palette,
        window: &Window,
        cx: &App,
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
                    Some(state) => {
                        let (rows, hunks) = self
                            .tool_diff_rows
                            .get(&(index, content_index))
                            .copied()
                            .unwrap_or((10, 1));
                        div()
                            .id(id)
                            .w_full()
                            .h(diff_height(rows, hunks, window, cx))
                            .child(Diff::new(state))
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
