use agent_client_protocol::schema::MaybeUndefined;
use agent_client_protocol::schema::v1::{SessionId, StateUpdate, SubagentUpdate};

#[derive(Clone, Debug)]
pub struct SubagentMessage {
    message_id: String,
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
        self.state.as_deref()
    }

    pub fn messages(&self) -> &[SubagentMessage] {
        &self.messages
    }

    pub fn upsert_message(&mut self, message_id: String, text: String) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == message_id) {
            msg.text = text;
        } else {
            self.messages.push(SubagentMessage { message_id, text });
        }
    }

    pub fn append_chunk(&mut self, message_id: String, text: &str) {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.message_id == message_id) {
            msg.text.push_str(text);
        } else {
            self.messages.push(SubagentMessage { message_id, text: text.into() });
        }
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
