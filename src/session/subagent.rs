use std::collections::HashMap;

use agent_client_protocol::schema::MaybeUndefined;
use agent_client_protocol::schema::v1::{
    ContentBlock, EmbeddedResourceResource, SessionId, SessionMessage, SessionMessageChunk,
    StateUpdate, SubagentUpdate,
};

#[derive(Clone, Debug)]
pub struct SubagentMessage {
    message_id: String,
    sender: Option<SessionId>,
    recipient: Option<SessionId>,
    text: String,
}

impl SubagentMessage {
    pub fn message_id(&self) -> &str {
        &self.message_id
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug)]
pub struct SubagentState {
    session_id: SessionId,
    title: Option<String>,
    description: Option<String>,
    state: Option<String>,
    messages: Vec<SubagentMessage>,
    message_index: HashMap<String, usize>,
}

impl SubagentState {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            title: None,
            description: None,
            state: None,
            messages: Vec::new(),
            message_index: HashMap::new(),
        }
    }

    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn state(&self) -> Option<&str> {
        self.state.as_deref()
    }

    pub fn messages(&self) -> &[SubagentMessage] {
        &self.messages
    }

    pub fn update_metadata(&mut self, update: SubagentUpdate) {
        apply_maybe_string(update.title, &mut self.title);
        apply_maybe_string(update.description, &mut self.description);
        match update.state {
            MaybeUndefined::Value(state) => self.state = Some(state_text(&state).into()),
            MaybeUndefined::Null => self.state = None,
            MaybeUndefined::Undefined => {}
        }
    }

    pub fn update_message(&mut self, update: SessionMessage) {
        let message_id = update.message_id.to_string();
        let message = self.message_mut(message_id);
        if update.sender_session_id.is_some() {
            message.sender = update.sender_session_id;
        }
        if update.recipient_session_id.is_some() {
            message.recipient = update.recipient_session_id;
        }
        match update.content {
            MaybeUndefined::Value(content) => {
                message.text = content.iter().map(content_block).collect::<Vec<_>>().join("\n\n");
            }
            MaybeUndefined::Null => message.text.clear(),
            MaybeUndefined::Undefined => {}
        }
    }

    pub fn append_message_chunk(&mut self, chunk: SessionMessageChunk) {
        let message_id = chunk.message_id.to_string();
        let message = self.message_mut(message_id);
        if chunk.sender_session_id.is_some() {
            message.sender = chunk.sender_session_id;
        }
        if chunk.recipient_session_id.is_some() {
            message.recipient = chunk.recipient_session_id;
        }
        let text = content_block(&chunk.content);
        message.text.push_str(&text);
    }

    fn message_mut(&mut self, message_id: String) -> &mut SubagentMessage {
        if let Some(index) = self.message_index.get(&message_id) {
            return &mut self.messages[*index];
        }
        let index = self.messages.len();
        self.messages.push(SubagentMessage {
            message_id: message_id.clone(),
            sender: None,
            recipient: None,
            text: String::new(),
        });
        self.message_index.insert(message_id, index);
        &mut self.messages[index]
    }
}

fn content_block(content: &ContentBlock) -> String {
    match content {
        ContentBlock::Text(content) => content.text.clone(),
        ContentBlock::Image(content) => format!("图片 · {}", content.mime_type),
        ContentBlock::Audio(content) => format!("音频 · {}", content.mime_type),
        ContentBlock::ResourceLink(resource) => {
            format!("资源：{}\n\n{}", resource.name, resource.uri)
        }
        ContentBlock::Resource(resource) => match &resource.resource {
            EmbeddedResourceResource::TextResourceContents(resource) => {
                format!("资源内容\n\n{}\n\n{}", resource.uri, resource.text)
            }
            EmbeddedResourceResource::BlobResourceContents(resource) => {
                format!(
                    "资源内容\n\n{}\n\n{}",
                    resource.uri,
                    resource.mime_type.as_deref().unwrap_or_default()
                )
            }
            _ => "资源内容".into(),
        },
        _ => "内容".into(),
    }
}

fn apply_maybe_string(value: MaybeUndefined<String>, target: &mut Option<String>) {
    match value {
        MaybeUndefined::Value(value) => *target = Some(value),
        MaybeUndefined::Null => *target = None,
        MaybeUndefined::Undefined => {}
    }
}

fn state_text(state: &StateUpdate) -> &'static str {
    match state {
        StateUpdate::Running(_) => "运行中",
        StateUpdate::Idle(_) => "空闲",
        StateUpdate::RequiresAction(_) => "等待操作",
        StateUpdate::Unknown(_) | StateUpdate::Other(_) => "未知",
        _ => "未知",
    }
}
