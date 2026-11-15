use std::path::{Path, PathBuf};

use aries_extension::AgentExtensions;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::{ScrollableElement, ScrollbarHandle};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Icon, Selectable, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;
use thiserror::Error;

use crate::workspace::skill_editor::{SkillEditor, SkillEditorEvent};
use crate::workspace::usage::ReturnToSessions;

const ENTRY_HEIGHT: f32 = 6.5;
const CATEGORIES: [&str; 4] = ["Skills", "Commands", "Hooks", "Agents"];
const CATEGORY_ICONS: [IconName; 4] =
    [IconName::BookOpen, IconName::Terminal, IconName::Zap, IconName::Bot];
const CATEGORY_DESCRIPTIONS: [&str; 4] = [
    "可按需加载的知识与工作流程",
    "可在会话中调用的自定义命令",
    "响应 Agent 事件的自动化配置",
    "具有独立指令与工具配置的子智能体",
];

struct ExtensionEntry {
    name: SharedString,
    description: SharedString,
    metadata: SharedString,
    location: SharedString,
    details: SharedString,
    is_skill: bool,
    path: PathBuf,
}

impl ExtensionEntry {
    fn new(name: &str, description: &str, metadata: String, location: &Path) -> Self {
        let path = location.to_owned();
        let location = location.display().to_string();
        let details = format!("{name}\n{description}\n{metadata}\n{location}").into();
        Self {
            name: name.to_owned().into(),
            description: description.to_owned().into(),
            metadata: metadata.into(),
            location: location.into(),
            details,
            is_skill: false,
            path,
        }
    }

    fn render(
        &self,
        owner: WeakEntity<ExtensionsView>,
        directory: Option<&Path>,
        cx: &App,
    ) -> AnyElement {
        let details = self.details.clone();
        let title = self.name.clone();
        let path = self.path.clone();
        let location = self.location.clone();
        let description = self.description.clone();
        let metadata = self.metadata.clone();
        let display_path = directory
            .and_then(|directory| self.path.strip_prefix(directory).ok())
            .unwrap_or(&self.path)
            .display()
            .to_string();
        div()
            .id(self.location.clone())
            .w_full()
            .h(rems(ENTRY_HEIGHT))
            .line_height(rems(1.25))
            .min_w_0()
            .px_4()
            .py_3()
            .hover(|row| row.bg(cx.theme().muted.opacity(0.4)))
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_weight(FontWeight::MEDIUM)
                            .child(summary(&self.name)),
                    )
                    .when(!self.is_skill, |row| {
                        row.child(
                            Button::new("extension-details")
                                .small()
                                .ghost()
                                .label("详情…")
                                .accessibility_label(format!("查看 {} 的详情", self.name))
                                .on_click(move |_, window, cx| {
                                    let title = title.clone();
                                    let details = details.clone();
                                    window.open_dialog(cx, move |dialog, _, cx| {
                                        dialog
                                            .title(title.clone())
                                            .centered()
                                            .width(cx.theme().font_size * 40.)
                                            .child(
                                                div()
                                                    .id("extension-details-content")
                                                    .max_h(rems(28.))
                                                    .overflow_y_scroll()
                                                    .flex()
                                                    .flex_col()
                                                    .gap_4()
                                                    .child(details.clone()),
                                            )
                                    });
                                }),
                        )
                    })
                    .when(self.is_skill, |row| {
                        row.child(
                            Button::new("edit-skill")
                                .ghost()
                                .small()
                                .label("编辑…")
                                .accessibility_label(format!("编辑 {}", self.name))
                                .on_click(move |_, window, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        view.edit_skill(path.clone(), window, cx)
                                    });
                                }),
                        )
                    }),
            )
            .child(
                div()
                    .id("extension-description")
                    .truncate()
                    .text_color(cx.theme().muted_foreground)
                    .child(summary(&self.description))
                    .tooltip(move |window, cx| Tooltip::new(description.clone()).build(window, cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .when(!self.metadata.is_empty(), |row| {
                        row.child(
                            div()
                                .id("extension-metadata")
                                .max_w(relative(0.5))
                                .truncate()
                                .child(summary(&self.metadata))
                                .tooltip(move |window, cx| {
                                    Tooltip::new(metadata.clone()).build(window, cx)
                                }),
                        )
                    })
                    .child(
                        div()
                            .id("extension-location")
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(summary(&display_path))
                            .tooltip(move |window, cx| {
                                Tooltip::new(location.clone()).build(window, cx)
                            }),
                    ),
            )
            .into_any_element()
    }
}

struct ExtensionData {
    groups: [Vec<ExtensionEntry>; 4],
}

impl ExtensionData {
    fn new(extensions: AgentExtensions) -> Result<Self, LoadExtensionsError> {
        let skills = extensions
            .skills
            .iter()
            .map(|skill| {
                let mut entry = ExtensionEntry::new(
                    &skill.frontmatter.name,
                    &skill.frontmatter.description,
                    if skill.frontmatter.allowed_tools.is_empty() {
                        "允许工具：未指定".to_owned()
                    } else {
                        format!(
                            "允许工具：{}",
                            skill.frontmatter.allowed_tools.as_slice().join(", ")
                        )
                    },
                    &skill.location,
                );
                entry.is_skill = true;
                entry
            })
            .collect();
        let commands = extensions
            .commands
            .iter()
            .map(|command| {
                ExtensionEntry::new(
                    &command.frontmatter.name,
                    &command.frontmatter.description,
                    command
                        .frontmatter
                        .argument_hint
                        .as_ref()
                        .map(|hint| format!("参数：{hint}"))
                        .unwrap_or_default(),
                    command.location(),
                )
            })
            .collect();
        let hooks = extensions
            .hooks
            .iter()
            .map(|definition| {
                let mut events: Vec<_> = definition
                    .hooks
                    .0
                    .iter()
                    .map(|(event, matchers)| {
                        let matchers = matchers
                            .iter()
                            .map(|matcher| {
                                format!(
                                    "{}（{} 条）",
                                    matcher
                                        .matcher
                                        .as_deref()
                                        .filter(|matcher| !matcher.is_empty() && *matcher != "*")
                                        .unwrap_or("全部"),
                                    matcher.hooks.len()
                                )
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        format!("{event:?}：{matchers}")
                    })
                    .collect();
                events.sort();
                let name = definition
                    .location
                    .file_name()
                    .unwrap_or(definition.location.as_os_str())
                    .to_string_lossy();
                let mut entry = ExtensionEntry::new(
                    &name,
                    definition.description.as_deref().unwrap_or("Hook 配置"),
                    events.join(" · "),
                    &definition.location,
                );
                let config = serde_json::to_string_pretty(&definition.hooks)
                    .map_err(LoadExtensionsError::serialize)?;
                entry.details = format!("{}\n\n{config}", entry.details).into();
                Ok(entry)
            })
            .collect::<Result<Vec<_>, LoadExtensionsError>>()?;
        let agents = extensions
            .agents
            .iter()
            .map(|agent| {
                ExtensionEntry::new(
                    &agent.frontmatter.name,
                    &agent.frontmatter.description,
                    format!(
                        "模型：{} · 工具：{}",
                        agent.frontmatter.model.as_deref().unwrap_or("继承"),
                        agent.frontmatter.tools_description()
                    ),
                    &agent.location,
                )
            })
            .collect();
        let mut groups: [Vec<ExtensionEntry>; 4] = [skills, commands, hooks, agents];
        for entries in &mut groups {
            entries.sort_by(|left, right| {
                left.name.cmp(&right.name).then_with(|| left.location.cmp(&right.location))
            });
        }
        Ok(Self { groups })
    }
}

pub struct ExtensionsView {
    directory: Option<PathBuf>,
    data: Option<ExtensionData>,
    category: usize,
    loading: bool,
    error: Option<String>,
    back_focus: FocusHandle,
    list_focus: FocusHandle,
    scroll: [UniformListScrollHandle; 4],
    _task: Option<Task<()>>,
    editor: Option<Entity<SkillEditor>>,
    _editor_subscription: Option<Subscription>,
}

impl EventEmitter<ReturnToSessions> for ExtensionsView {}

impl ExtensionsView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            directory: None,
            data: None,
            category: 0,
            loading: false,
            error: None,
            back_focus: cx.focus_handle().tab_stop(true),
            list_focus: cx.focus_handle().tab_stop(true),
            scroll: std::array::from_fn(|_| UniformListScrollHandle::new()),
            _task: None,
            editor: None,
            _editor_subscription: None,
        }
    }

    pub fn open(&mut self, directory: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(editor) = &self.editor {
            editor.update(cx, |editor, cx| editor.focus(window, cx));
        } else {
            self.back_focus.focus(window, cx);
        }
        if self.directory.as_ref() != Some(&directory) {
            self.directory = Some(directory);
            self.data = None;
            self.refresh(window, cx);
        } else if self.data.is_none() && !self.loading {
            self.refresh(window, cx);
        }
    }

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(directory) = self.directory.clone() else { return };
        self._task = None;
        self.loading = true;
        self.error = None;
        let background = cx.background_executor().spawn(async move { load_extensions(&directory) });
        self._task = Some(cx.spawn_in(window, async move |this, cx| {
            let result = background.await;
            let _ = this.update_in(cx, |this, _, cx| {
                this.loading = false;
                match result {
                    Ok(data) => this.data = Some(data),
                    Err(error) => {
                        crate::error::log("无法读取 Agent 扩展", &error);
                        this.error = Some(crate::error::details(&error));
                    },
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn edit_skill(&mut self, location: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        let editor = cx.new(|cx| SkillEditor::new(location, window, cx));
        editor.update(cx, |editor, cx| editor.focus(window, cx));
        self._editor_subscription =
            Some(cx.subscribe_in(&editor, window, |this, editor, event, window, cx| {
                match event {
                    SkillEditorEvent::Saved => this.refresh(window, cx),
                    SkillEditorEvent::Closed => {
                        let restore_focus = editor.read(cx).is_focused(window, cx);
                        this.editor = None;
                        this._editor_subscription = None;
                        if restore_focus {
                            this.back_focus.focus(window, cx);
                        }
                    },
                }
                cx.notify();
            }));
        self.editor = Some(editor);
        cx.notify();
    }

    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let directory =
            self.directory.as_deref().map(|path| path.display().to_string()).unwrap_or_default();
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap_3()
            .px_3()
            .py_2()
            .child(
                Button::new("extensions-back")
                    .ghost()
                    .small()
                    .track_focus(&self.back_focus)
                    .icon(IconName::ArrowLeft)
                    .accessibility_label("返回会话")
                    .tooltip("返回会话")
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ReturnToSessions::new()))),
            )
            .child(div().font_weight(FontWeight::SEMIBOLD).child("Agent 扩展"))
            .child(
                div()
                    .id("extensions-project")
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(directory.clone())
                    .tooltip(move |window, cx| Tooltip::new(directory.clone()).build(window, cx)),
            )
    }

    fn render_navigation(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(rems(10.5))
            .min_w(rems(8.5))
            .max_w(relative(0.3))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .py_4()
            .child(div().px_2().text_xs().text_color(cx.theme().muted_foreground).child("本地扩展"))
            .child(div().flex().flex_col().gap_1().children(CATEGORIES.iter().enumerate().map(
                |(ix, label)| {
                    let count = self.data.as_ref().map(|data| data.groups[ix].len());
                    Button::new(("extension-category", ix))
                        .ghost()
                        .small()
                        .w_full()
                        .justify_start()
                        .selected(self.category == ix)
                        .rounded(cx.theme().radius_tokens().md)
                        .accessibility_label(*label)
                        .when(self.category == ix, |button| {
                            button
                                .bg(cx.theme().secondary)
                                .text_color(cx.theme().secondary_foreground)
                                .font_weight(FontWeight::SEMIBOLD)
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .w_full()
                                .gap_2()
                                .child(Icon::new(CATEGORY_ICONS[ix]).size_4())
                                .child(div().flex_1().min_w_0().truncate().child(*label))
                                .when_some(count, |row, count| {
                                    row.child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(count.to_string()),
                                    )
                                }),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.category = ix;
                            cx.notify();
                        }))
                },
            )))
    }

    fn render_content_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap_3()
            .px_4()
            .py_3()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().font_weight(FontWeight::SEMIBOLD).child(CATEGORIES[self.category]))
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(CATEGORY_DESCRIPTIONS[self.category]),
                    ),
            )
            .child(
                Button::new("refresh-extensions")
                    .ghost()
                    .small()
                    .label(if self.loading { "读取中" } else { "刷新" })
                    .icon(IconName::RefreshCw)
                    .loading(self.loading)
                    .on_click(cx.listener(|this, _, window, cx| this.refresh(window, cx))),
            )
    }

    fn scroll_entries(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.data.as_ref().is_none_or(|data| data.groups[self.category].is_empty()) {
            return;
        }
        let key = &event.keystroke;
        if key.modifiers.platform
            || key.modifiers.control
            || key.modifiers.alt
            || key.modifiers.shift
        {
            return;
        }
        let scroll = &self.scroll[self.category];
        let viewport = scroll.viewport_bounds().size.height;
        let maximum = (scroll.content_size().height - viewport).max(Pixels::ZERO);
        let mut offset = scroll.offset();
        let step = window.rem_size() * ENTRY_HEIGHT;
        offset.y = match key.key.as_str() {
            "up" => offset.y + step,
            "down" => offset.y - step,
            "pageup" => offset.y + viewport,
            "pagedown" => offset.y - viewport,
            "home" => Pixels::ZERO,
            "end" => -maximum,
            _ => return,
        }
        .clamp(-maximum, Pixels::ZERO);
        scroll.set_offset(offset);
        self.list_focus.focus(window, cx);
        cx.stop_propagation();
        cx.notify();
    }

    fn render_entries(&self, cx: &mut Context<Self>) -> AnyElement {
        let count = self.data.as_ref().map(|data| data.groups[self.category].len());
        if count.is_none_or(|count| count == 0) {
            let (title, description) = if self.loading {
                ("正在读取扩展…".to_owned(), "读取用户与项目的本地定义。")
            } else if self.error.is_some() {
                ("无法读取扩展".to_owned(), "请刷新重试。")
            } else {
                (
                    format!("暂无 {}", CATEGORIES[self.category]),
                    "在用户或项目的 .agents 目录添加定义后刷新。",
                )
            };
            return div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .text_center()
                .gap_2()
                .p_6()
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(Icon::new(CATEGORY_ICONS[self.category]).size_6()),
                )
                .child(div().font_weight(FontWeight::MEDIUM).child(title))
                .child(div().text_xs().text_color(cx.theme().muted_foreground).child(description))
                .into_any_element();
        }
        let owner = cx.entity();
        let category = self.category;
        uniform_list(
            ("extension-list", category),
            count.unwrap_or_default(),
            move |range, _, cx| {
                let view = owner.read(cx);
                let Some(data) = &view.data else { return Vec::new() };
                range
                    .filter_map(|ix| data.groups[category].get(ix))
                    .map(|entry| entry.render(owner.downgrade(), view.directory.as_deref(), cx))
                    .collect()
            },
        )
        .track_scroll(&self.scroll[self.category])
        .size_full()
        .into_any_element()
    }
}

impl Render for ExtensionsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(editor) = &self.editor {
            return div().id("extensions-page").size_full().child(editor.clone());
        }
        div()
            .id("extensions-page")
            .size_full()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .text_sm()
            .bg(cx.theme().muted.opacity(0.25))
            .text_color(cx.theme().foreground)
            .child(self.render_header(cx))
            .child(
                div()
                    .flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .child(self.render_navigation(cx))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .mr_1p5()
                            .mb_1p5()
                            .flex()
                            .flex_col()
                            .bg(cx.theme().background)
                            .rounded(cx.theme().radius_tokens().lg)
                            .overflow_hidden()
                            .child(self.render_content_header(cx))
                            .when_some(self.error.as_ref(), |content, error| {
                                content.child(
                                    div()
                                        .px_4()
                                        .py_2()
                                        .text_xs()
                                        .text_color(cx.theme().danger)
                                        .child(format!("读取失败：{error} 请刷新重试。")),
                                )
                            })
                            .child(
                                div()
                                    .id("extension-list-viewport")
                                    .relative()
                                    .flex_1()
                                    .min_h_0()
                                    .min_w_0()
                                    .track_focus(&self.list_focus)
                                    .aria_label(format!("{} 列表", CATEGORIES[self.category]))
                                    .aria_description("方向键与 Page Up/Down 滚动，Home/End 跳转首尾，Tab 访问条目操作。")
                                    .border_1()
                                    .border_color(cx.theme().transparent)
                                    .focus_visible(|style| style.border_color(cx.theme().ring))
                                    .on_key_down(cx.listener(Self::scroll_entries))
                                    .child(self.render_entries(cx))
                                    .when(
                                        self.data.as_ref().is_some_and(|data| !data.groups[self.category].is_empty()),
                                        |list| list.vertical_scrollbar(&self.scroll[self.category]),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .px_4()
                                    .py_2()
                                    .border_t_1()
                                    .border_color(cx.theme().border)
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("仅显示已加载的本地定义；解析失败的文件会被跳过。此页不会执行扩展。"),
                            ),
                    ),
            )
    }
}

fn summary(text: &str) -> String {
    text.split(['\r', '\n'])
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Error)]
enum LoadExtensionsError {
    #[error("无法确定 Aries 数据目录")]
    HomeUnavailable,
    #[error("无法启动扩展加载运行时")]
    Runtime { source: std::io::Error },
    #[error("无法读取 Hook 配置")]
    Serialize { source: serde_json::Error },
}

impl LoadExtensionsError {
    fn home_unavailable() -> Self {
        Self::HomeUnavailable
    }

    fn runtime(source: std::io::Error) -> Self {
        Self::Runtime { source }
    }

    fn serialize(source: serde_json::Error) -> Self {
        Self::Serialize { source }
    }
}

fn load_extensions(directory: &Path) -> Result<ExtensionData, LoadExtensionsError> {
    let home = std::env::home_dir().ok_or_else(LoadExtensionsError::home_unavailable)?;
    let root = home.join(".local/share/aries");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(LoadExtensionsError::runtime)?;
    ExtensionData::new(runtime.block_on(AgentExtensions::new(directory, root)))
}
