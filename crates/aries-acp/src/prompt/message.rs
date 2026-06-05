use agent_client_protocol::schema::ContentBlock;
use rig_core::OneOrMany;
use rig_core::agent::Text;
use rig_core::completion::Message;
use rig_core::message::{Audio, DocumentSourceKind, Image, UserContent};

pub struct UserMessage(Message);

impl From<Vec<ContentBlock>> for UserMessage {
    fn from(value: Vec<ContentBlock>) -> Self {
        let contents = value
            .into_iter()
            .filter_map(|block| match block {
                ContentBlock::Text(t) => Some(UserContent::Text(Text { text: t.text })),
                ContentBlock::Image(i) => Some(UserContent::Image(Image {
                    data: DocumentSourceKind::String(i.data),
                    ..Default::default()
                })),
                ContentBlock::Audio(a) => Some(UserContent::Audio(Audio {
                    data: DocumentSourceKind::String(a.data),
                    ..Default::default()
                })),
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
