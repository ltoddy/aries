use crate::workspace::conversation::composer::*;

impl SessionView {
    pub fn open_attachment_preview(
        &mut self,
        id: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
                    .map_err(|source| LoadAttachmentsError::selection_wait(source.into()))?
                    .map_err(|source| LoadAttachmentsError::selection_open(source.into()))?
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
                let name = format!("粘贴图片.{}", image.format().extension());
                let error = if !capabilities.image {
                    AttachmentError::image_unsupported(name)
                } else if image.bytes().is_empty() {
                    AttachmentError::empty(name)
                } else {
                    AttachmentError::image_format_unsupported(
                        name,
                        image.format().mime_type().to_owned(),
                    )
                };
                images.push(AttachmentSource::from_error(LoadAttachmentError::image(error)));
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
        sources: impl Future<Output = Result<Vec<AttachmentSource>, LoadAttachmentsError>> + 'static,
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
        result: Result<AttachmentBatch, LoadAttachmentsError>,
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
                            Err(error) => report("无法添加附件", &error, window, cx),
                        }
                    }
                },
                Err(error) => report("无法添加附件", &error, window, cx),
            }
        });
        cx.notify();
    }
}

#[derive(Debug, Error)]
enum LoadAttachmentsError {
    #[error("等待文件选择失败")]
    SelectionWait { source: Box<dyn std::error::Error + Send + Sync> },
    #[error("打开文件选择窗口失败")]
    SelectionOpen { source: Box<dyn std::error::Error + Send + Sync> },
}

impl LoadAttachmentsError {
    fn selection_wait(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::SelectionWait { source }
    }

    fn selection_open(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::SelectionOpen { source }
    }
}

#[derive(Debug, Error)]
enum LoadAttachmentError {
    #[error("无法添加 {}", path.display())]
    Path { path: PathBuf, source: AttachmentError },
    #[error("无法添加粘贴图片")]
    Image { source: AttachmentError },
}

impl LoadAttachmentError {
    fn path(path: PathBuf, source: AttachmentError) -> Self {
        Self::Path { path, source }
    }

    fn image(source: AttachmentError) -> Self {
        Self::Image { source }
    }
}

enum AttachmentSource {
    Path(PathBuf),
    Image(Image),
    Error(LoadAttachmentError),
}

impl AttachmentSource {
    fn from_path(path: impl Into<PathBuf>) -> Self {
        Self::Path(path.into())
    }

    fn from_image(image: Image) -> Self {
        Self::Image(image)
    }

    fn from_error(error: LoadAttachmentError) -> Self {
        Self::Error(error)
    }

    fn load(
        self,
        capabilities: &PromptCapabilities,
    ) -> Result<PromptAttachment, LoadAttachmentError> {
        match self {
            Self::Error(error) => Err(error),
            Self::Path(path) => PromptAttachment::from_path(&path, capabilities)
                .map_err(|source| LoadAttachmentError::path(path, source)),
            Self::Image(image) => {
                let name = format!("粘贴图片.{}", image.format().extension());
                PromptAttachment::from_image(name, image, None, capabilities)
                    .map_err(LoadAttachmentError::image)
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
    attachments: Vec<Result<PromptAttachment, LoadAttachmentError>>,
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
