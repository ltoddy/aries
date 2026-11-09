use agent_client_protocol::v1::schema::{ToolCall, ToolCallStatus, ToolCallUpdateFields};

#[derive(Clone, Debug)]
pub struct ToolCallState {
    call: ToolCall,
    interrupted: bool,
}

impl ToolCallState {
    pub fn new(call: ToolCall) -> Self {
        Self { call, interrupted: false }
    }

    pub fn call(&self) -> &ToolCall {
        &self.call
    }

    pub fn update(&mut self, update: ToolCallUpdateFields) {
        if update.status.is_some() {
            self.update_status();
        }
        self.call.update(update);
    }

    pub fn replace(&mut self, call: ToolCall) {
        self.update_status();
        self.call = call;
    }

    fn update_status(&mut self) {
        self.interrupted = false;
    }

    pub fn title(&self) -> &str {
        if !self.call.title.trim().is_empty() {
            &self.call.title
        } else {
            self.call.name.as_deref().filter(|name| !name.trim().is_empty()).unwrap_or("工具调用")
        }
    }

    pub fn is_interrupted(&self) -> bool {
        self.interrupted
    }

    pub fn finish(&mut self) {
        self.interrupted =
            matches!(self.call.status, ToolCallStatus::Pending | ToolCallStatus::InProgress);
    }
}
