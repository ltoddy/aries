use std::future::Future;
use std::path::PathBuf;

use agent_client_protocol::v1::schema::{AvailableCommand, ContentBlock, PromptCapabilities};
use anyhow::Context as _;
use gpui_kit::assets::IconName;
use gpui_kit::base::motion::{Presence, Transition};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{
    Enter, Escape, IndentInline, InputEvent, MoveDown, MoveUp, Textarea, TextareaState,
};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::conversation::draft::{
    Draft, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS, PromptAttachment,
};
use crate::workspace::conversation::slash_commands::{CommandMenu, CommandMenuAction, SlashMenu};
use crate::workspace::settings::SendKey;
use crate::workspace::settings::palette::Palette;

pub struct Composer {
    input: Entity<TextareaState>,
    command_menu: SlashMenu,
    attachments: Vec<PromptAttachment>,
    attachment_task: Option<Task<()>>,
    attachment_revision: u64,
}

impl Composer {
    pub fn new(window: &mut Window, cx: &mut Context<SessionView>) -> Self {
        let input =
            cx.new(|cx| TextareaState::new(window, cx).placeholder("发送消息…").auto_grow(2, 6));
        Self {
            input,
            command_menu: SlashMenu::default(),
            attachments: Vec::new(),
            attachment_task: None,
            attachment_revision: 0,
        }
    }
    pub fn input(&self) -> &Entity<TextareaState> {
        &self.input
    }
    pub fn is_loading(&self) -> bool {
        self.attachment_task.is_some()
    }
    pub fn has_content(&self, cx: &App) -> bool {
        self.attachment_task.is_none()
            && (!self.input.read(cx).value().trim().is_empty() || !self.attachments.is_empty())
    }

    pub fn content(&self, cx: &App) -> Vec<ContentBlock> {
        let text = self.input.read(cx).value();
        let mut content = Vec::new();
        if !text.trim().is_empty() {
            content.push(ContentBlock::from(text.trim()));
        }
        content.extend(self.attachments.iter().map(|attachment| attachment.content().clone()));
        content
    }

    pub fn draft(&self, cx: &App) -> Draft {
        Draft::new(self.input.read(cx).value().to_string(), self.attachments.clone())
    }

    pub fn restore_draft(
        &mut self,
        draft: Draft,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let (text, attachments) = draft.into_parts();
        self.clear_attachments();
        self.attachments = attachments;
        self.input.update(cx, |input, cx| input.set_value(text, window, cx));
    }

    pub fn clear(&mut self, window: &mut Window, cx: &mut Context<SessionView>) {
        self.clear_attachments();
        self.reset_menu();
        self.input.update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
    }

    pub fn clear_attachments(&mut self) {
        self.attachment_revision = self.attachment_revision.wrapping_add(1);
        self.attachment_task = None;
        self.attachments.clear();
    }

    fn render_attachments(&self, p: Palette, cx: &mut Context<SessionView>) -> AnyElement {
        div()
            .id("draft-attachments")
            .max_h_24()
            .min_h_0()
            .overflow_y_scrollbar()
            .flex()
            .flex_wrap()
            .gap_2()
            .children(self.attachments.iter().map(|attachment| {
                let id = attachment.id();
                let name = attachment.name().to_owned();
                let icon = if attachment.mime_type().starts_with("image/") {
                    IconName::Image
                } else if attachment.mime_type().starts_with("audio/") {
                    IconName::FileMusic
                } else {
                    IconName::FileText
                };
                div()
                    .id(("draft-attachment", id))
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_2()
                    .w_64()
                    .flex_shrink_0()
                    .max_w_full()
                    .min_w_0()
                    .rounded(cx.theme().radius_lg)
                    .border_1()
                    .border_color(p.border())
                    .bg(p.input())
                    .child(match attachment.image() {
                        Some(image) => Button::new(("attachment-thumbnail", id))
                            .ghost()
                            .size_12()
                            .p_1()
                            .flex_shrink_0()
                            .accessibility_label(format!("预览图片：{}", attachment.name()))
                            .child(
                                img(image)
                                    .size_10()
                                    .rounded(cx.theme().radius)
                                    .object_fit(ObjectFit::Contain)
                                    .with_fallback(move || Icon::new(icon).into_any_element()),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open_attachment_preview(id, window, cx);
                            }))
                            .into_any_element(),
                        None => Icon::new(icon).size_6().into_any_element(),
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .id(("attachment-name", id))
                                    .truncate()
                                    .text_sm()
                                    .child(name.clone())
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(name.clone()).build(window, cx)
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_1()
                                    .text_xs()
                                    .text_color(p.muted())
                                    .child(format!("{:.1} KiB", attachment.size() as f64 / 1024.))
                                    .when(attachment.image().is_some(), |row| {
                                        row.child(
                                            Button::new(("preview-attachment", id))
                                                .ghost()
                                                .xsmall()
                                                .label("预览…")
                                                .on_click(cx.listener(
                                                    move |this, _, window, cx| {
                                                        this.open_attachment_preview(
                                                            id, window, cx,
                                                        );
                                                    },
                                                )),
                                        )
                                    }),
                            ),
                    )
                    .child(
                        Button::new(("remove-attachment", id))
                            .ghost()
                            .small()
                            .icon(IconName::X)
                            .accessibility_label(format!("移除附件：{}", attachment.name()))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.with_composer(|composer| {
                                    composer.attachments.retain(|attachment| attachment.id() != id)
                                });
                                this.input().update(cx, |input, cx| input.focus(window, cx));
                                cx.notify();
                            })),
                    )
            }))
            .when(self.attachment_task.is_some(), |row| {
                row.child(div().text_sm().text_color(p.muted()).child("正在添加附件…"))
            })
            .into_any_element()
    }

    pub fn reset_menu(&mut self) {
        self.command_menu.reset();
    }
    pub fn input_event(&mut self, event: &InputEvent) {
        match event {
            InputEvent::Change => self.command_menu.reset(),
            InputEvent::Focus => self.command_menu.focus(),
            InputEvent::Blur => self.command_menu.blur(),
            _ => {},
        }
    }

    fn command_candidates<'a>(
        &self,
        commands: &'a [AvailableCommand],
        available: bool,
        cx: &App,
    ) -> Option<Vec<&'a AvailableCommand>> {
        if !self.command_menu.is_open() || !available {
            return None;
        }
        let input = self.input.read(cx);
        let text = input.value();
        let query = text.strip_prefix('/')?;
        if query.chars().any(|character| character.is_whitespace() || character == '/')
            || !input.selected_range().is_empty()
            || input.cursor() != text.len()
        {
            return None;
        }
        let query = query.to_lowercase();
        Some(
            commands
                .iter()
                .filter(|command| command.name.to_lowercase().starts_with(&query))
                .collect(),
        )
    }

    pub fn complete_selected_command(
        &mut self,
        commands: &[AvailableCommand],
        available: bool,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) -> bool {
        let Some(candidates) = self.command_candidates(commands, available, cx) else {
            return false;
        };
        let Some(command) = candidates.get(self.command_menu.selected()) else {
            return false;
        };
        self.select_command(&command.name, window, cx);
        true
    }

    pub fn select_command(
        &mut self,
        name: &str,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let text = format!("/{name} ");
        let end = text.len();
        self.input.update(cx, |input, cx| {
            input.replace_all(text, window, cx);
            input.set_selected_range(end..end, cx);
            input.focus(window, cx);
        });
        self.command_menu.dismiss();
        cx.notify();
    }

    pub fn command_menu_action(
        &mut self,
        action: CommandMenuAction,
        commands: &[AvailableCommand],
        available: bool,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let Some(count) =
            self.command_candidates(commands, available, cx).map(|candidates| candidates.len())
        else {
            return;
        };
        if self.input.update(cx, |input, cx| input.marked_text_range(window, cx).is_some()) {
            return;
        }
        match action {
            CommandMenuAction::Previous if count > 0 => {
                self.command_menu.move_selection(action, count);
            },
            CommandMenuAction::Next if count > 0 => {
                self.command_menu.move_selection(action, count);
            },
            CommandMenuAction::Complete if count > 0 => {
                self.complete_selected_command(commands, available, window, cx);
            },
            CommandMenuAction::Dismiss => self.command_menu.dismiss(),
            _ => return,
        }
        window.prevent_default();
        cx.stop_propagation();
        cx.notify();
    }

    pub fn render_commands(
        &self,
        commands: &[AvailableCommand],
        available: bool,
        has_session: bool,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> Option<AnyElement> {
        let candidates = self.command_candidates(commands, available, cx)?;
        let empty = if !has_session {
            "新建会话后可获取可用命令"
        } else if commands.is_empty() {
            "当前会话暂无可用命令"
        } else {
            "没有匹配的命令"
        };
        let view = cx.weak_entity();
        Some(
            CommandMenu::from_commands(
                &self.command_menu,
                candidates,
                empty,
                p,
                move |name, window, cx| {
                    let _ = view.update(cx, |this, cx| this.select_command(name, window, cx));
                },
            )
            .into_any_element(),
        )
    }
    pub fn render(
        &self,
        session: &SessionView,
        p: Palette,
        cx: &mut Context<SessionView>,
    ) -> impl IntoElement {
        let can_send = session.can_send(cx);
        let send_key = session.send_key(cx);
        div().flex_shrink_0().pl_8().pr_4().py_2().bg(p.background()).child(
            div()
                .w_full()
                .max_w(rems(72.))
                .mx_auto()
                .flex()
                .flex_col()
                .gap_2()
                .when_some(session.execution().permission(), |composer, permission| {
                    composer.child(PermissionPrompt::new(
                        ("permission-motion", cx.entity_id()).into(),
                        div()
                            .p_3()
                            .rounded(cx.theme().radius_lg)
                            .border_1()
                            .border_color(p.border())
                            .bg(p.input())
                            .child(
                                div()
                                    .mb_2()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(permission.title().to_owned()),
                            )
                            .child(div().flex().flex_wrap().gap_2().children(
                                permission.options().iter().enumerate().map(|(index, option)| {
                                    let id = option.option_id.clone();
                                    Button::new(("permission", index))
                                        .label(option.name.clone())
                                        .small()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.choose_permission(id.clone(), cx)
                                        }))
                                }),
                            ))
                            .into_any_element(),
                    ))
                })
                .child(session.render_execution_context(p, cx))
                .when_some(session.execution().prompt_queue(), |composer, queue| {
                    composer.child(queue.render(session, p, cx))
                })
                .child(
                    div()
                        .relative()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .capture_action(cx.listener(|this, _: &MoveUp, window, cx| {
                            this.command_menu_action(CommandMenuAction::Previous, window, cx)
                        }))
                        .capture_action(cx.listener(|this, _: &MoveDown, window, cx| {
                            this.command_menu_action(CommandMenuAction::Next, window, cx)
                        }))
                        .capture_action(cx.listener(|this, action: &Enter, window, cx| {
                            if !this.input().read(cx).focus_handle(cx).is_focused(window)
                                || this.input().update(cx, |input, cx| {
                                    input.marked_text_range(window, cx).is_some()
                                })
                            {
                                return;
                            }
                            if !action.shift && !action.secondary {
                                this.command_menu_action(CommandMenuAction::Complete, window, cx);
                                if window.default_prevented() {
                                    return;
                                }
                            }
                            if !action.shift
                                && action.secondary == (this.send_key(cx) == SendKey::CommandEnter)
                            {
                                window.prevent_default();
                                cx.stop_propagation();
                                this.send(window, cx);
                            }
                        }))
                        .capture_action(cx.listener(|this, _: &IndentInline, window, cx| {
                            this.command_menu_action(CommandMenuAction::Complete, window, cx)
                        }))
                        .capture_action(cx.listener(|this, _: &Escape, window, cx| {
                            this.command_menu_action(CommandMenuAction::Dismiss, window, cx)
                        }))
                        .children(self.render_commands(
                            session.execution().conversation().available_commands(),
                            session.commands_available(),
                            session.execution().session_id().is_some(),
                            p,
                            cx,
                        ))
                        .child(self.render_attachments(p, cx))
                        .child(ComposerInput::new(self, session, p, cx.weak_entity(), can_send)),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .min_h_6()
                        .child(div().flex_1().min_w_0().when(
                            !session.execution().conversation().config_options().is_empty()
                                || session.configuration().error().is_some(),
                            |footer| footer.child(session.render_session_config(p, cx)),
                        ))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_xs()
                                .text_color(p.muted())
                                .text_right()
                                .child(format!(
                                    "{} {} · {} 换行",
                                    send_key.send_label(),
                                    if session.execution().conversation().is_busy() {
                                        "加入队列"
                                    } else if session.execution().prompt_queue().is_some() {
                                        "合并发送"
                                    } else {
                                        "发送"
                                    },
                                    send_key.newline_label(),
                                )),
                        ),
                ),
        )
    }
}

impl SessionView {
    fn open_attachment_preview(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some((name, image)) = self.with_composer(|composer| {
            let attachment =
                composer.attachments.iter().find(|attachment| attachment.id() == id)?;
            Some((attachment.name().to_owned(), attachment.image()?))
        }) else {
            return;
        };
        window.open_dialog(cx, move |dialog, _, cx| {
            let image = image.clone();
            dialog
                .title(div().min_w_0().pr_6().truncate().child(name.clone()))
                .centered()
                .width(cx.theme().font_size * 44.)
                .content(move |content, window, cx| {
                    content
                        .min_h_0()
                        .h((window.viewport_size().height * 0.65).min(cx.theme().font_size * 32.))
                        .child(
                            img(image.clone())
                                .size_full()
                                .object_fit(ObjectFit::Contain)
                                .with_fallback(|| {
                                    div()
                                        .size_full()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child("无法加载图片")
                                        .into_any_element()
                                }),
                        )
                })
        });
    }

    pub fn choose_attachments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.attachments_available() || self.composer_loading() {
            return;
        }
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("添加图片、音频、PDF 或文本".into()),
        });
        self.load_attachments(
            async move {
                let paths = selection
                    .await
                    .context("等待文件选择失败")?
                    .context("打开文件选择窗口失败")?
                    .unwrap_or_default();
                Ok(paths.into_iter().map(AttachmentSource::from_path).collect())
            },
            window,
            cx,
        );
    }

    pub fn attach_paths(
        &mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut paths = paths.into_iter().peekable();
        if paths.peek().is_none() {
            return;
        }
        if !self.attachments_available() || self.composer_loading() {
            window
                .push_notification(Notification::warning("当前无法添加附件，请等待操作结束。"), cx);
            return;
        }
        let sources = paths.map(AttachmentSource::from_path).collect();
        self.load_attachments(async move { Ok(sources) }, window, cx);
    }

    pub fn paste_attachments(
        &mut self,
        item: &ClipboardItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let has_media = item.entries.iter().any(|entry| {
            matches!(entry, ClipboardEntry::Image(_) | ClipboardEntry::ExternalPaths(_))
        });
        if !has_media {
            return false;
        }
        if !self.attachments_available() || self.composer_loading() {
            window
                .push_notification(Notification::warning("当前无法添加附件，请等待操作结束。"), cx);
            return true;
        }
        let paths: Vec<_> = item
            .entries
            .iter()
            .filter_map(|entry| match entry {
                ClipboardEntry::ExternalPaths(paths) => Some(paths.0.iter().cloned()),
                _ => None,
            })
            .flatten()
            .collect();
        if !paths.is_empty() {
            self.attach_paths(paths, window, cx);
            return true;
        }
        let (count, mut bytes) = self.with_composer(|composer| {
            (
                composer.attachments.len(),
                composer.attachments.iter().map(PromptAttachment::size).sum::<usize>(),
            )
        });
        let mut images = Vec::new();
        let mut image_count = 0;
        let mut reserved_count = count;
        let capabilities = self.execution().prompt_capabilities();
        let mut size_exceeded = false;
        for entry in &item.entries {
            let ClipboardEntry::Image(image) = entry else {
                continue;
            };
            image_count += 1;
            if image_count > MAX_ATTACHMENTS || reserved_count >= MAX_ATTACHMENTS {
                continue;
            }
            if !capabilities.image
                || !matches!(
                    image.format(),
                    ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Webp | ImageFormat::Gif
                )
                || image.bytes().is_empty()
            {
                let message = if !capabilities.image {
                    "当前 Agent 不支持图片附件"
                } else if image.bytes().is_empty() {
                    "不能添加空文件"
                } else {
                    "仅支持 PNG、JPEG、WebP 和 GIF 图片"
                };
                images.push(AttachmentSource::from_error(anyhow::anyhow!(message)));
                continue;
            }
            if image.bytes().len() > MAX_ATTACHMENT_BYTES.saturating_sub(bytes) {
                size_exceeded = true;
                continue;
            }
            bytes += image.bytes().len();
            reserved_count += 1;
            images.push(AttachmentSource::from_image(image.clone()));
        }
        if image_count > MAX_ATTACHMENTS.saturating_sub(count) {
            window.push_notification(Notification::warning(attachment_count_warning()), cx);
        }
        if size_exceeded {
            window.push_notification(Notification::warning(attachment_size_warning()), cx);
        }
        self.load_attachments(async move { Ok(images) }, window, cx);
        true
    }

    fn load_attachments(
        &mut self,
        sources: impl Future<Output = anyhow::Result<Vec<AttachmentSource>>> + 'static,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input().update(cx, |input, cx| input.focus(window, cx));
        let capabilities = self.execution().prompt_capabilities().clone();
        let session_id = self.execution().session_id().cloned();
        let (revision, count, bytes) = self.with_composer(|composer| {
            composer.attachment_revision = composer.attachment_revision.wrapping_add(1);
            (
                composer.attachment_revision,
                composer.attachments.len(),
                composer.attachments.iter().map(PromptAttachment::size).sum(),
            )
        });
        let task = cx.spawn_in(window, async move |view, cx| {
            let result = match sources.await {
                Ok(sources) => Ok(cx
                    .background_executor()
                    .spawn(
                        async move { load_attachment_batch(sources, &capabilities, count, bytes) },
                    )
                    .await),
                Err(error) => Err(error),
            };
            let _ = view.update_in(cx, |this, window, cx| {
                if this.execution().session_id() == session_id.as_ref()
                    && this.with_composer(|composer| composer.attachment_revision == revision)
                {
                    this.finish_attachments(result, window, cx);
                }
            });
        });
        self.with_composer(|composer| composer.attachment_task = Some(task));
        cx.notify();
    }

    fn composer_loading(&mut self) -> bool {
        self.with_composer(|composer| composer.is_loading())
    }

    fn finish_attachments(
        &mut self,
        result: anyhow::Result<AttachmentBatch>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.with_composer(|composer| {
            composer.attachment_task = None;
            match result {
                Ok(batch) => {
                    if batch.count_exceeded {
                        window.push_notification(
                            Notification::warning(attachment_count_warning()),
                            cx,
                        );
                    }
                    if batch.size_exceeded {
                        window.push_notification(
                            Notification::warning(attachment_size_warning()),
                            cx,
                        );
                    }
                    for result in batch.attachments {
                        match result {
                            Ok(attachment) => composer.attachments.push(attachment),
                            Err(error) => window
                                .push_notification(Notification::error(format!("{error:#}")), cx),
                        }
                    }
                },
                Err(error) => window
                    .push_notification(Notification::error(format!("无法添加附件：{error:#}")), cx),
            }
        });
        cx.notify();
    }
}

enum AttachmentSource {
    Path(PathBuf),
    Image(Image),
    Error(anyhow::Error),
}

impl AttachmentSource {
    fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }

    fn from_image(image: Image) -> Self {
        Self::Image(image)
    }

    fn from_error(error: anyhow::Error) -> Self {
        Self::Error(error)
    }

    fn load(self, capabilities: &PromptCapabilities) -> anyhow::Result<PromptAttachment> {
        match self {
            Self::Error(error) => Err(error),
            Self::Path(path) => PromptAttachment::from_path(&path, capabilities)
                .with_context(|| format!("无法添加 {}", path.display())),
            Self::Image(image) => {
                let name = format!("粘贴图片.{}", image.format().extension());
                PromptAttachment::from_image(name, image, None, capabilities)
            },
        }
    }
}

fn attachment_count_warning() -> String {
    format!("每条消息最多 {MAX_ATTACHMENTS} 个附件，超出部分未添加。")
}

fn attachment_size_warning() -> String {
    format!("每条消息的附件总大小不能超过 {} MiB。", MAX_ATTACHMENT_BYTES / (1024 * 1024))
}

struct AttachmentBatch {
    attachments: Vec<anyhow::Result<PromptAttachment>>,
    count_exceeded: bool,
    size_exceeded: bool,
}

impl AttachmentBatch {
    fn new(count_exceeded: bool) -> Self {
        Self { attachments: Vec::new(), count_exceeded, size_exceeded: false }
    }
}

fn load_attachment_batch(
    sources: Vec<AttachmentSource>,
    capabilities: &PromptCapabilities,
    mut count: usize,
    mut bytes: usize,
) -> AttachmentBatch {
    let mut batch = AttachmentBatch::new(sources.len() > MAX_ATTACHMENTS.saturating_sub(count));
    for source in sources.into_iter().take(MAX_ATTACHMENTS) {
        if count >= MAX_ATTACHMENTS {
            batch.count_exceeded = true;
            break;
        }
        match source.load(capabilities) {
            Ok(attachment) => {
                if attachment.size() > MAX_ATTACHMENT_BYTES.saturating_sub(bytes) {
                    batch.size_exceeded = true;
                    continue;
                }
                count += 1;
                bytes += attachment.size();
                batch.attachments.push(Ok(attachment));
            },
            Err(error) => batch.attachments.push(Err(error)),
        }
    }
    batch
}

#[derive(IntoElement)]
struct PermissionPrompt {
    id: ElementId,
    child: AnyElement,
}

impl PermissionPrompt {
    fn new(id: ElementId, child: AnyElement) -> Self {
        Self { id, child }
    }
}

impl RenderOnce for PermissionPrompt {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let motion = cx.theme().motion_tokens();
        let distance = motion.distance_short;
        let transition =
            Transition::new(motion.duration_normal).easing(motion.easing_enter.clone());
        let presence = Presence::new(self.id, true).transition(transition).sample(window, cx);
        div().w_full().child(
            div()
                .relative()
                .top(distance * (1. - presence.progress))
                .opacity(presence.progress)
                .child(self.child),
        )
    }
}

#[derive(IntoElement)]
struct ComposerInput {
    input: Entity<TextareaState>,
    busy: bool,
    stopping: bool,
    can_send: bool,
    palette: Palette,
    attachments_available: bool,
    view: WeakEntity<SessionView>,
    has_queue: bool,
}
impl ComposerInput {
    fn new(
        composer: &Composer,
        session: &SessionView,
        palette: Palette,
        view: WeakEntity<SessionView>,
        can_send: bool,
    ) -> Self {
        Self {
            input: composer.input.clone(),
            busy: session.execution().conversation().is_busy(),
            stopping: session.execution().is_stopping(),
            can_send,
            palette,
            attachments_available: session.attachments_available()
                && composer.attachment_task.is_none(),
            view,
            has_queue: session.execution().prompt_queue().is_some(),
        }
    }
}

impl RenderOnce for ComposerInput {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let Self {
            input,
            busy,
            stopping,
            can_send,
            palette: p,
            attachments_available,
            view,
            has_queue,
        } = self;
        let picker = view.clone();
        let drop_view = view.clone();
        let stop_view = view.clone();
        let send_view = view.clone();
        div()
            .id("multimodal-composer")
            .on_drop(move |paths: &ExternalPaths, window, cx| {
                let _ = drop_view
                    .update(cx, |this, cx| this.attach_paths(paths.0.iter().cloned(), window, cx));
            })
            .when(attachments_available, |composer| {
                composer.drag_over::<ExternalPaths>(|style, _, _, cx| {
                    style.border_color(cx.theme().primary)
                })
            })
            .flex()
            .items_end()
            .gap_2()
            .p_1()
            .rounded(rems(0.875))
            .border_1()
            .border_color(p.border())
            .bg(p.input())
            .child(
                Button::new("add-attachments")
                    .ghost()
                    .icon(IconName::Paperclip)
                    .accessibility_label("添加附件…")
                    .tooltip("添加图片、音频、PDF 或文本…")
                    .size_10()
                    .disabled(!attachments_available)
                    .on_click(move |_, window, cx| {
                        let _ = picker.update(cx, |this, cx| this.choose_attachments(window, cx));
                    }),
            )
            .child(
                div().id("composer-input").flex_1().min_w_0().px_1().child(
                    Textarea::new(&input)
                        .appearance(false)
                        .bordered(false)
                        .aria_label("消息")
                        .w_full()
                        .on_paste(move |item, window, cx| {
                            view.update(cx, |this, cx| this.paste_attachments(item, window, cx))
                                .unwrap_or(false)
                        }),
                ),
            )
            .when(busy, |composer| {
                composer.child(
                    Button::new("stop")
                        .ghost()
                        .icon(IconName::Square)
                        .label(if stopping { "停止中" } else { "停止" })
                        .accessibility_label("停止回复")
                        .disabled(stopping)
                        .on_click(move |_, window, cx| {
                            let _ = stop_view.update(cx, |this, cx| this.stop(window, cx));
                        }),
                )
            })
            .child(
                Button::new("send")
                    .primary()
                    .icon(IconName::Send)
                    .when(!busy && !has_queue, |button| {
                        button.size_10().rounded(cx.theme().radius_lg)
                    })
                    .when(busy || has_queue, |button| {
                        button.label(if busy { "加入队列" } else { "合并发送" })
                    })
                    .accessibility_label(if busy {
                        "加入待发送队列"
                    } else if has_queue {
                        "合并队列与消息发送"
                    } else {
                        "发送消息"
                    })
                    .disabled(!can_send)
                    .on_click(move |_, window, cx| {
                        let _ = send_view.update(cx, |this, cx| this.send(window, cx));
                    }),
            )
    }
}
