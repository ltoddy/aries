use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_client_protocol::v1::schema::{
    AudioContent, BlobResourceContents, ContentBlock, EmbeddedResource, EmbeddedResourceResource,
    ImageContent, PromptCapabilities, TextResourceContents,
};
use anyhow::{Context as _, bail};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use gpui_kit::{Image, ImageFormat};
use url::Url;

pub const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_ATTACHMENTS: usize = 10;
static NEXT_ATTACHMENT_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Default)]
pub struct Draft {
    text: String,
    attachments: Vec<PromptAttachment>,
}

impl Draft {
    pub fn new(text: String, attachments: Vec<PromptAttachment>) -> Self {
        Self { text, attachments }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty() && self.attachments.is_empty()
    }

    pub fn into_parts(self) -> (String, Vec<PromptAttachment>) {
        (self.text, self.attachments)
    }
}

#[derive(Clone)]
pub struct PromptAttachment {
    id: u64,
    name: String,
    mime_type: String,
    content: Arc<ContentBlock>,
    image: Option<Arc<Image>>,
    size: usize,
}

impl PromptAttachment {
    fn new(
        name: String,
        mime_type: String,
        content: ContentBlock,
        image: Option<Arc<Image>>,
        size: usize,
    ) -> Self {
        Self {
            id: NEXT_ATTACHMENT_ID.fetch_add(1, Ordering::Relaxed),
            name,
            mime_type,
            content: Arc::new(content),
            image,
            size,
        }
    }

    pub fn from_path(path: &Path, capabilities: &PromptCapabilities) -> anyhow::Result<Self> {
        let path = path.canonicalize().with_context(|| format!("无法访问 {}", path.display()))?;
        let file = File::open(&path).with_context(|| format!("无法读取 {}", path.display()))?;
        if !file.metadata()?.is_file() {
            bail!("请选择文件，不支持文件夹");
        }
        let mut bytes = Vec::new();
        file.take(MAX_ATTACHMENT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
        validate_size(bytes.len())?;
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let uri = Url::from_file_path(&path)
            .map_err(|_| anyhow::anyhow!("无法转换文件路径"))?
            .to_string();
        let extension = path.extension().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
        let mime_type = match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "flac" => "audio/flac",
            "ogg" => "audio/ogg",
            "m4a" => "audio/m4a",
            "aac" => "audio/aac",
            "pdf" => "application/pdf",
            "md" | "markdown" => "text/markdown",
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "csv" => "text/csv",
            "json" => "application/json",
            "xml" => "application/xml",
            "js" | "mjs" => "application/javascript",
            "py" => "text/x-python",
            _ => "text/plain",
        };
        let size = bytes.len();
        if mime_type.starts_with("image/") {
            let format = ImageFormat::from_mime_type(mime_type)
                .ok_or_else(|| anyhow::anyhow!("不支持此图片格式"))?;
            return Self::from_image(
                name,
                Image::from_bytes(format, bytes),
                Some(uri),
                capabilities,
            );
        }
        let content = if mime_type.starts_with("audio/") {
            if !capabilities.audio {
                bail!("当前 Agent 不支持音频附件");
            }
            ContentBlock::Audio(AudioContent::new(STANDARD.encode(bytes), mime_type).meta(
                serde_json::Map::from_iter([(
                    String::from("name"),
                    serde_json::Value::String(name.clone()),
                )]),
            ))
        } else {
            if !capabilities.embedded_context {
                bail!("当前 Agent 不支持文档附件");
            }
            let resource = if mime_type == "application/pdf" {
                if !bytes.starts_with(b"%PDF-") {
                    bail!("文件不是有效的 PDF");
                }
                EmbeddedResourceResource::BlobResourceContents(
                    BlobResourceContents::new(STANDARD.encode(bytes), uri)
                        .mime_type(mime_type.to_owned()),
                )
            } else {
                let text = String::from_utf8(bytes)
                    .context("仅支持 UTF-8 文本或 PDF 文档，不支持此二进制文件")?;
                if text.contains('\0') {
                    bail!("不支持此二进制文件，请选择图片、音频、PDF 或文本");
                }
                EmbeddedResourceResource::TextResourceContents(
                    TextResourceContents::new(text, uri).mime_type(mime_type.to_owned()),
                )
            };
            ContentBlock::Resource(EmbeddedResource::new(resource))
        };
        Ok(Self::new(name, mime_type.to_owned(), content, None, size))
    }

    pub fn from_image(
        name: String,
        image: Image,
        uri: Option<String>,
        capabilities: &PromptCapabilities,
    ) -> anyhow::Result<Self> {
        if !capabilities.image {
            bail!("当前 Agent 不支持图片附件");
        }
        if !matches!(
            image.format(),
            ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Webp | ImageFormat::Gif
        ) {
            bail!("仅支持 PNG、JPEG、WebP 和 GIF 图片");
        }
        validate_size(image.bytes().len())?;
        let mime_type = image.format().mime_type().to_owned();
        let size = image.bytes().len();
        let content = ContentBlock::Image(
            ImageContent::new(STANDARD.encode(image.bytes()), mime_type.clone()).uri(uri),
        );
        Ok(Self::new(name, mime_type, content, Some(Arc::new(image)), size))
    }

    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }
    pub fn content(&self) -> &ContentBlock {
        &self.content
    }
    pub fn image(&self) -> Option<Arc<Image>> {
        self.image.clone()
    }
    pub fn size(&self) -> usize {
        self.size
    }
}

fn validate_size(size: usize) -> anyhow::Result<()> {
    if size == 0 {
        bail!("不能添加空文件");
    }
    if size > MAX_ATTACHMENT_BYTES {
        bail!("附件不能超过 20 MiB");
    }
    Ok(())
}
