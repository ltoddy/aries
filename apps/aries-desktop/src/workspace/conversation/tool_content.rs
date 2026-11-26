use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use agent_client_protocol::v1::schema::{SessionUpdate, ToolCallContent, ToolCallLocation};
use gpui_kit::assets::IconName;
use gpui_kit::base::SelectableText;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::TextView;
use gpui_kit::component::{ActiveTheme, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::conversation::attachment::{Attachment, resource_uri};
use crate::workspace::conversation::model::Conversation;
use crate::workspace::conversation::model::tool_call::ToolCallState;
use crate::workspace::conversation::text::{
    DisclosureChevron, file_button, link_handler, single_line,
};
use crate::workspace::files::OpenFile;
use crate::workspace::settings::palette::Palette;
use crate::workspace::settings::typography;

#[derive(IntoElement)]
struct ToolContent {
    index: usize,
    entries: Vec<AnyElement>,
    locations: Vec<ToolCallLocation>,
    session: WeakEntity<SessionView>,
    palette: Palette,
}

impl ToolContent {
    fn new(
        index: usize,
        entries: Vec<AnyElement>,
        locations: Vec<ToolCallLocation>,
        session: WeakEntity<SessionView>,
        palette: Palette,
    ) -> Self {
        Self { index, entries, locations, session, palette }
    }
}

impl RenderOnce for ToolContent {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { index, entries, locations, session, palette: p } = self;
        let directory =
            session.upgrade().map(|session| session.read(cx).execution().directory().to_owned());
        let mut seen = HashSet::new();
        let mut file_buttons = Vec::new();
        if let Some(directory) = directory.as_ref() {
            for location in &locations {
                if location.path.as_os_str().is_empty() {
                    continue;
                }
                let line = location
                    .line
                    .and_then(|line| usize::try_from(line).ok())
                    .and_then(NonZeroUsize::new);
                if !seen.insert((location.path.as_path(), line)) {
                    continue;
                }
                let target = OpenFile::new(directory.clone(), location.path.clone());
                let target = match line {
                    Some(line) => target.with_line(line),
                    None => target,
                };
                let path = single_line(location.path.to_string_lossy());
                let label = match line {
                    Some(line) => format!("{path}:{line}"),
                    None => path,
                };
                file_buttons.push(file_button(
                    SharedString::from(format!(
                        "tool-location:{index}:{:?}:{line:?}",
                        location.path
                    )),
                    label,
                    target,
                    session.clone(),
                ));
            }
        }
        div()
            .id(("tool-details", index))
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .items_start()
            .gap_1()
            .text_sm()
            .text_color(p.muted())
            .children(file_buttons)
            .children(entries)
    }
}

enum DiffContent {
    Markdown(SharedString),
    Path(String),
}

impl DiffContent {
    fn markdown(text: SharedString) -> Self {
        Self::Markdown(text)
    }

    fn path(path: String) -> Self {
        Self::Path(path)
    }
}

#[derive(IntoElement)]
struct ToolDiff {
    id: SharedString,
    index: usize,
    content_index: usize,
    content: DiffContent,
    path: PathBuf,
    expanded: bool,
    session: WeakEntity<SessionView>,
    line: Option<NonZeroUsize>,
}

impl ToolDiff {
    fn new(
        id: SharedString,
        index: usize,
        content_index: usize,
        content: DiffContent,
        path: PathBuf,
        expanded: bool,
        session: WeakEntity<SessionView>,
    ) -> Self {
        Self { id, index, content_index, content, path, expanded, session, line: None }
    }

    fn with_line(mut self, line: Option<NonZeroUsize>) -> Self {
        self.line = line;
        self
    }
}

impl RenderOnce for ToolDiff {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self { id, index, content_index, content, path, expanded, session, line } = self;
        let target = session.upgrade().and_then(|session| {
            (!path.as_os_str().is_empty()).then(|| {
                OpenFile::new(session.read(cx).execution().directory().to_owned(), path.clone())
            })
        });
        let target = target.map(|target| match line {
            Some(line) => target.with_line(line),
            None => target,
        });
        let button = target.map(|target| {
            file_button(
                SharedString::from(format!("{id}:open-file")),
                single_line(path.to_string_lossy()),
                target,
                session.clone(),
            )
        });
        let (markdown, label) = match content {
            DiffContent::Markdown(markdown) => (Some(markdown), None),
            DiffContent::Path(path) => (None, Some(path)),
        };
        let toggle = markdown.as_ref().map(|_| {
            let view = session.clone();
            Button::new(SharedString::from(format!("{id}:toggle")))
                .ghost()
                .xsmall()
                .compact()
                .accessibility_label(if expanded { "收起改动" } else { "展开改动" })
                .on_click(move |_, _, cx| {
                    let _ = view.update(cx, |session, cx| {
                        session.with_timeline(|timeline, _, _| {
                            timeline.toggle_diff(index, content_index);
                        });
                        cx.notify();
                    });
                })
                .child(DisclosureChevron::new(id.clone(), expanded))
        });
        let body = match (markdown, expanded) {
            (Some(markdown), true) => Some(
                TextView::markdown(id, markdown)
                    .w_full()
                    .min_w_0()
                    .map(typography::style_markdown)
                    .selectable(true)
                    .scrollable(false)
                    .on_link_click(link_handler(session))
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
                                        .text_color(cx.theme().muted_foreground)
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
                                            .text_color(cx.theme().muted_foreground),
                                    )
                            })
                            .into_any_element()
                    })
                    .into_any_element(),
            ),
            (None, _) => label
                .filter(|_| button.is_none())
                .map(|path| SelectableText::new(id, path).into_any_element()),
            _ => None,
        };
        div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .items_start()
            .gap_1()
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_1()
                    .children(toggle)
                    .children(button),
            )
            .children(body)
    }
}

pub struct ToolDiffs {
    patches: HashMap<(usize, usize), SharedString>,
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
                    self.patches.insert((index, content_index), SharedString::from(markdown));
                }
            }
        }
    }

    pub fn render(
        &self,
        index: usize,
        tool: &ToolCallState,
        p: Palette,
        session: WeakEntity<SessionView>,
        expanded: &HashSet<(usize, usize)>,
    ) -> AnyElement {
        let diff_paths: HashSet<&Path> = tool
            .call()
            .content
            .iter()
            .filter_map(|content| match content {
                ToolCallContent::Diff(diff) => Some(diff.path.as_path()),
                _ => None,
            })
            .collect();
        let entries = tool
            .call()
            .content
            .iter()
            .enumerate()
            .filter_map(|(content_index, content)| {
                if let ToolCallContent::Content(content) = content {
                    resource_uri(&content.content)?;
                    return Some(
                        Attachment::from_content(index, content_index, &content.content, None, p)
                            .with_session(session.clone())
                            .into_any_element(),
                    );
                }
                let ToolCallContent::Diff(diff) = content else {
                    return None;
                };
                let line = tool.call().locations.iter().find_map(|location| {
                    (location.path == diff.path)
                        .then_some(location.line)
                        .flatten()
                        .and_then(|line| usize::try_from(line).ok())
                        .and_then(NonZeroUsize::new)
                });
                Some(
                    ToolDiff::new(
                        SharedString::from(format!("tool-content:{index}:{content_index}")),
                        index,
                        content_index,
                        self.patches
                            .get(&(index, content_index))
                            .cloned()
                            .map(DiffContent::markdown)
                            .unwrap_or_else(|| {
                                DiffContent::path(diff.path.to_string_lossy().into_owned())
                            }),
                        diff.path.clone(),
                        expanded.contains(&(index, content_index)),
                        session.clone(),
                    )
                    .with_line(line)
                    .into_any_element(),
                )
            })
            .collect();
        let locations = tool
            .call()
            .locations
            .iter()
            .filter(|location| !diff_paths.contains(location.path.as_path()))
            .cloned()
            .collect();
        ToolContent::new(index, entries, locations, session, p).into_any_element()
    }
}
