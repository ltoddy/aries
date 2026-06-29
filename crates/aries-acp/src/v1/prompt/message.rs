use agent_client_protocol::schema::v1::{ContentBlock, EmbeddedResourceResource};
use rig_core::OneOrMany;
use rig_core::completion::Message;
use rig_core::message::{DocumentMediaType, UserContent};

pub struct UserMessage(Message);

impl From<Vec<ContentBlock>> for UserMessage {
    fn from(value: Vec<ContentBlock>) -> Self {
        let contents = value
            .into_iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(UserContent::text(t.text)),
                ContentBlock::Image(i) => Some(UserContent::image_base64(i.data, None, None)),
                ContentBlock::Audio(a) => Some(UserContent::audio(a.data, None)),
                ContentBlock::ResourceLink(link) => Some(UserContent::document_url(link.uri, None)),
                ContentBlock::Resource(resource) => Some(match resource.resource {
                    EmbeddedResourceResource::TextResourceContents(text) => UserContent::document(
                        text.text,
                        text.mime_type.and_then(document_media_type),
                    ),
                    EmbeddedResourceResource::BlobResourceContents(blob) => {
                        UserContent::document_raw(
                            blob.blob,
                            blob.mime_type.and_then(document_media_type),
                        )
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
