use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_client_protocol::v1::schema::{
    AudioContent, BlobResourceContents, ContentBlock, EmbeddedResource, EmbeddedResourceResource,
    ImageContent, PromptCapabilities, TextResourceContents,
};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use gpui_kit::{Image, ImageFormat};
use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum AttachmentError {
    #[error("无法访问 {}", path.display())]
    Access { path: PathBuf, source: std::io::Error },
    #[error("无法打开 {}", path.display())]
    Open { path: PathBuf, source: std::io::Error },
    #[error("无法获取文件信息 {}", path.display())]
    Metadata { path: PathBuf, source: std::io::Error },
    #[error("无法读取 {}", path.display())]
    Read { path: PathBuf, source: std::io::Error },
    #[error("请选择文件，不支持文件夹：{}", path.display())]
    NotFile { path: PathBuf },
    #[error("无法转换文件路径：{}", path.display())]
    FileUri { path: PathBuf },
    #[error("不能添加空文件：{name}")]
    Empty { name: String },
    #[error("附件不能超过 20 MiB：{name}（{size} 字节）")]
    TooLarge { name: String, size: usize },
    #[error("当前 Agent 不支持图片附件：{name}")]
    ImageUnsupported { name: String },
    #[error("当前 Agent 不支持音频附件：{name}")]
    AudioUnsupported { name: String },
    #[error("当前 Agent 不支持文档附件：{name}")]
    DocumentUnsupported { name: String },
    #[error("仅支持 PNG、JPEG、WebP 和 GIF 图片：{name}（{mime_type}）")]
    ImageFormatUnsupported { name: String, mime_type: String },
    #[error("文件不是有效的 PDF：{}", path.display())]
    InvalidPdf { path: PathBuf },
    #[error("仅支持 UTF-8 文本或 PDF 文档，不支持此二进制文件：{}", path.display())]
    Utf8 { path: PathBuf, source: std::string::FromUtf8Error },
    #[error("不支持此二进制文件，请选择图片、音频、PDF 或文本：{}", path.display())]
    Binary { path: PathBuf },
}

impl AttachmentError {
    fn access(path: &Path, source: std::io::Error) -> Self {
        Self::Access { path: path.to_owned(), source }
    }

    fn open(path: &Path, source: std::io::Error) -> Self {
        Self::Open { path: path.to_owned(), source }
    }

    fn metadata(path: &Path, source: std::io::Error) -> Self {
        Self::Metadata { path: path.to_owned(), source }
    }

    fn read(path: &Path, source: std::io::Error) -> Self {
        Self::Read { path: path.to_owned(), source }
    }

    fn not_file(path: &Path) -> Self {
        Self::NotFile { path: path.to_owned() }
    }

    fn file_uri(path: &Path) -> Self {
        Self::FileUri { path: path.to_owned() }
    }

    pub fn empty(name: String) -> Self {
        Self::Empty { name }
    }

    fn too_large(name: String, size: usize) -> Self {
        Self::TooLarge { name, size }
    }

    pub fn image_unsupported(name: String) -> Self {
        Self::ImageUnsupported { name }
    }

    fn audio_unsupported(name: String) -> Self {
        Self::AudioUnsupported { name }
    }

    fn document_unsupported(name: String) -> Self {
        Self::DocumentUnsupported { name }
    }

    pub fn image_format_unsupported(name: String, mime_type: String) -> Self {
        Self::ImageFormatUnsupported { name, mime_type }
    }

    fn invalid_pdf(path: &Path) -> Self {
        Self::InvalidPdf { path: path.to_owned() }
    }

    fn utf8(path: &Path, source: std::string::FromUtf8Error) -> Self {
        Self::Utf8 { path: path.to_owned(), source }
    }

    fn binary(path: &Path) -> Self {
        Self::Binary { path: path.to_owned() }
    }
}

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

    pub fn from_path(
        path: impl AsRef<Path>,
        capabilities: &PromptCapabilities,
    ) -> Result<Self, AttachmentError> {
        let path = path.as_ref();
        let path = path.canonicalize().map_err(|source| AttachmentError::access(path, source))?;
        let file = File::open(&path).map_err(|source| AttachmentError::open(&path, source))?;
        if !file.metadata().map_err(|source| AttachmentError::metadata(&path, source))?.is_file() {
            return Err(AttachmentError::not_file(&path));
        }
        let mut bytes = Vec::new();
        file.take(MAX_ATTACHMENT_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|source| AttachmentError::read(&path, source))?;
        let name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        validate_size(&name, bytes.len())?;
        let uri =
            Url::from_file_path(&path).map_err(|_| AttachmentError::file_uri(&path))?.to_string();
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
            let format = ImageFormat::from_mime_type(mime_type).ok_or_else(|| {
                AttachmentError::image_format_unsupported(name.clone(), mime_type.to_owned())
            })?;
            return Self::from_image(
                name,
                Image::from_bytes(format, bytes),
                Some(uri),
                capabilities,
            );
        }
        let content = if mime_type.starts_with("audio/") {
            if !capabilities.audio {
                return Err(AttachmentError::audio_unsupported(name));
            }
            ContentBlock::Audio(AudioContent::new(STANDARD.encode(bytes), mime_type).meta(
                serde_json::Map::from_iter([(
                    String::from("name"),
                    serde_json::Value::String(name.clone()),
                )]),
            ))
        } else {
            if !capabilities.embedded_context {
                return Err(AttachmentError::document_unsupported(name));
            }
            let resource = if mime_type == "application/pdf" {
                if !bytes.starts_with(b"%PDF-") {
                    return Err(AttachmentError::invalid_pdf(&path));
                }
                EmbeddedResourceResource::BlobResourceContents(
                    BlobResourceContents::new(STANDARD.encode(bytes), uri)
                        .mime_type(mime_type.to_owned()),
                )
            } else {
                let text = String::from_utf8(bytes)
                    .map_err(|source| AttachmentError::utf8(&path, source))?;
                if text.contains('\0') {
                    return Err(AttachmentError::binary(&path));
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
    ) -> Result<Self, AttachmentError> {
        if !capabilities.image {
            return Err(AttachmentError::image_unsupported(name));
        }
        if !matches!(
            image.format(),
            ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Webp | ImageFormat::Gif
        ) {
            return Err(AttachmentError::image_format_unsupported(
                name,
                image.format().mime_type().to_owned(),
            ));
        }
        validate_size(&name, image.bytes().len())?;
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

fn validate_size(name: &str, size: usize) -> Result<(), AttachmentError> {
    if size == 0 {
        return Err(AttachmentError::empty(name.to_owned()));
    }
    if size > MAX_ATTACHMENT_BYTES {
        return Err(AttachmentError::too_large(name.to_owned(), size));
    }
    Ok(())
}
