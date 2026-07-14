use agent_client_protocol::schema::v1::{ContentBlock, EmbeddedResourceResource};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rig_core::OneOrMany;
use rig_core::completion::Message;
use rig_core::message::{Document, DocumentMediaType, DocumentSourceKind, UserContent};

pub struct UserMessage(Message);

impl From<Vec<ContentBlock>> for UserMessage {
    fn from(value: Vec<ContentBlock>) -> Self {
        let contents = value
            .into_iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(UserContent::text(t.text)),
                ContentBlock::Image(i) => Some(UserContent::image_base64(i.data, None, None)),
                ContentBlock::Audio(a) => Some(UserContent::audio(a.data, None)),
                ContentBlock::ResourceLink(link) => {
                    let media_type = link.mime_type.and_then(document_media_type);
                    let content = match &link.description {
                        Some(desc) => format!("[{}]({}): {}", link.name, link.uri, desc),
                        None => format!("[{}]({})", link.name, link.uri),
                    };
                    Some(UserContent::document(content, media_type))
                },
                ContentBlock::Resource(resource) => Some(match resource.resource {
                    EmbeddedResourceResource::TextResourceContents(text) => UserContent::document(
                        text.text,
                        text.mime_type.and_then(document_media_type),
                    ),
                    EmbeddedResourceResource::BlobResourceContents(blob) => {
                        let media_type = blob.mime_type.and_then(document_media_type);

                        // PDF 需要保持 base64 encode之后的形式, 其他类型需要使用明文来让 AI 模型阅读
                        match media_type {
                            Some(DocumentMediaType::PDF) => UserContent::Document(Document {
                                data: DocumentSourceKind::Base64(blob.blob),
                                media_type,
                                additional_params: None,
                            }),
                            _ => match STANDARD.decode(&blob.blob) {
                                Ok(bytes) => {
                                    let text = String::from_utf8_lossy(&bytes).into_owned();
                                    UserContent::document(text, media_type)
                                },
                                Err(_) => UserContent::Document(Document {
                                    data: DocumentSourceKind::Base64(blob.blob),
                                    media_type,
                                    additional_params: None,
                                }),
                            },
                        }
                    },
                    _ => UserContent::text("Attached resource of unsupported type"),
                }),
                _ => None,
            })
            .collect::<Vec<UserContent>>();

        if contents.is_empty() {
            return UserMessage("".into());
        }

        UserMessage(Message::User { content: OneOrMany::many(contents).unwrap() })
    }
}

impl From<UserMessage> for Message {
    fn from(value: UserMessage) -> Self {
        value.0
    }
}

// TODO media type
fn document_media_type(mime_type: impl Into<String>) -> Option<DocumentMediaType> {
    let mime_type = mime_type.into();
    let mime_type = mime_type.as_str();

    let mime_type = mime_type.split(';').next().unwrap_or(mime_type).trim();
    let media_type = match mime_type {
        "application/pdf" => DocumentMediaType::PDF,
        "text/plain" => DocumentMediaType::TXT,
        "application/rtf" | "text/rtf" => DocumentMediaType::RTF,
        "text/html" => DocumentMediaType::HTML,
        "text/css" => DocumentMediaType::CSS,
        "text/markdown" => DocumentMediaType::MARKDOWN,
        "text/csv" => DocumentMediaType::CSV,
        "application/xml" | "text/xml" => DocumentMediaType::XML,
        "application/javascript" | "text/javascript" => DocumentMediaType::Javascript,
        "text/x-python" | "application/x-python-code" => DocumentMediaType::Python,
        _ => return None,
    };
    Some(media_type)
}
