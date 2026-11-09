use agent_client_protocol::v1::schema::{MessageId, SessionId, StateUpdate, SubagentUpdate};

#[derive(Clone, Debug)]
pub struct SubagentMessage {
    message_id: MessageId,
    text: String,
}

impl SubagentMessage {
    pub fn message_id(&self) -> &MessageId {
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
    state: Option<StateUpdate>,
    messages: Vec<SubagentMessage>,
}

impl SubagentState {
    pub fn new(session_id: SessionId) -> Self {
        Self { session_id, title: None, description: None, state: None, messages: Vec::new() }
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
        self.state.as_ref().map(state_text)
    }

    pub fn is_idle(&self) -> bool {
        matches!(self.state, Some(StateUpdate::Idle(_)))
    }

    pub fn messages(&self) -> &[SubagentMessage] {
        &self.messages
    }

    pub fn upsert_message(&mut self, message_id: MessageId, text: String) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == message_id) {
            msg.text = text;
        } else {
            self.messages.push(SubagentMessage { message_id, text });
        }
    }

    pub fn append_chunk(&mut self, message_id: &MessageId, text: &str) {
        if let Some(msg) = self.messages.iter_mut().find(|m| &m.message_id == message_id) {
            msg.text.push_str(text);
        } else {
            self.messages
                .push(SubagentMessage { message_id: message_id.clone(), text: text.into() });
        }
    }

    pub fn update_metadata(&mut self, update: SubagentUpdate) {
        update.title.update_to(&mut self.title);
        update.description.update_to(&mut self.description);
        update.state.update_to(&mut self.state);
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
