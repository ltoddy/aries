use agent_client_protocol::v1::schema::{ContentBlock, EmbeddedResourceResource};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rig::completion::Message;
use rig::message::{AudioMediaType, DocumentMediaType, ImageMediaType, MimeType, UserContent};

#[derive(Debug, Clone)]
pub struct UserMessage(Message);

impl From<Vec<ContentBlock>> for UserMessage {
    fn from(value: Vec<ContentBlock>) -> Self {
        let content = value
            .into_iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(UserContent::text(t.text)),
                ContentBlock::Image(i) => Some(UserContent::image_base64(
                    i.data,
                    ImageMediaType::from_mime_type(&i.mime_type),
                    None,
                )),
                ContentBlock::Audio(a) => Some(UserContent::audio_base64(
                    a.data,
                    AudioMediaType::from_mime_type(&a.mime_type),
                )),
                ContentBlock::ResourceLink(link) => {
                    let media_type =
                        link.mime_type.as_deref().and_then(DocumentMediaType::from_mime_type);
                    let content = match &link.description {
                        Some(desc) => format!("[{}]({}): {}", link.name, link.uri, desc),
                        None => format!("[{}]({})", link.name, link.uri),
                    };
                    Some(UserContent::document_text(content, media_type))
                },
                ContentBlock::Resource(resource) => Some(match resource.resource {
                    EmbeddedResourceResource::TextResourceContents(text) => {
                        UserContent::document_text(
                            text.text,
                            text.mime_type.as_deref().and_then(DocumentMediaType::from_mime_type),
                        )
                    },
                    EmbeddedResourceResource::BlobResourceContents(blob) => {
                        let media_type =
                            blob.mime_type.as_deref().and_then(DocumentMediaType::from_mime_type);

                        // PDF 需要保持 base64 encode之后的形式, 其他类型需要使用明文来让 AI 模型阅读
                        match media_type {
                            Some(DocumentMediaType::PDF) => {
                                UserContent::document_base64(blob.blob, media_type)
                            },
                            _ => match STANDARD.decode(&blob.blob) {
                                Ok(bytes) => {
                                    let text = String::from_utf8_lossy(&bytes).into_owned();
                                    UserContent::document_text(text, media_type)
                                },
                                Err(_) => UserContent::document_base64(blob.blob, media_type),
                            },
                        }
                    },
                    _ => UserContent::text("Attached resource of unsupported type"),
                }),
                _ => None,
            })
            .collect::<Vec<UserContent>>();

        UserMessage(Message::User { content })
    }
}

impl From<UserMessage> for Message {
    fn from(value: UserMessage) -> Self {
        value.0
    }
}
