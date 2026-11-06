use std::collections::HashMap;

use agent_client_protocol::schema::MaybeUndefined;
use agent_client_protocol::schema::v1::{
    AvailableCommand, CompactionStatus, CompactionUpdate, ContentBlock, ContentChunk,
    SessionConfigOption, SessionId, SessionNotification, SessionUpdate, ToolCall, ToolCallId,
};

use crate::session::display::UpdateDisplay;
use crate::session::subagent::SubagentState;
use crate::session::tool_call::ToolCallState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    Thought,
    Event,
}

#[derive(Clone, Debug)]
pub struct Message {
    role: Role,
    title: String,
    text: String,
    updates: Vec<SessionNotification>,
    compaction: Option<CompactionUpdate>,
    tool_call: Option<ToolCallState>,
    subagent: Option<SubagentState>,
}

impl Message {
    pub fn tool_call(&self) -> Option<&ToolCallState> {
        self.tool_call.as_ref()
    }
    pub fn role(&self) -> Role {
        self.role
    }
    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn updates(&self) -> &[SessionNotification] {
        &self.updates
    }
    pub fn attachments(&self) -> impl Iterator<Item = (usize, &ContentBlock)> {
        self.updates.iter().enumerate().filter_map(|(index, notification)| {
            match &notification.update {
                SessionUpdate::UserMessageChunk(chunk)
                    if !matches!(chunk.content, ContentBlock::Text(_)) =>
                {
                    Some((index, &chunk.content))
                }
                _ => None,
            }
        })
    }
    pub fn compaction(&self) -> Option<&CompactionUpdate> {
        self.compaction.as_ref()
    }
    pub fn subagent(&self) -> Option<&SubagentState> {
        self.subagent.as_ref()
    }
}

#[derive(Debug, Default)]
pub struct Chat {
    messages: Vec<Message>,
    busy: bool,
    error: Option<String>,
    session_title: Option<String>,
    available_commands: Vec<AvailableCommand>,
    config_options: Vec<SessionConfigOption>,
    stream: Option<usize>,
    tools: HashMap<(SessionId, ToolCallId), usize>,
    subagents: HashMap<SessionId, usize>,
    subagent_message_route: HashMap<(SessionId, String), SessionId>,
    pending_subagent_chunks: HashMap<(SessionId, String), Vec<String>>,
}

impl Chat {
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }
    pub fn is_busy(&self) -> bool {
        self.busy
    }
    pub fn is_streaming(&self, index: usize) -> bool {
        self.stream == Some(index)
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn available_commands(&self) -> &[AvailableCommand] {
        &self.available_commands
    }

    pub fn config_options(&self) -> &[SessionConfigOption] {
        &self.config_options
    }

    pub fn set_config_options(&mut self, options: Vec<SessionConfigOption>) {
        self.config_options = options;
    }

    pub fn begin_turn(&mut self, text: &str) -> Option<String> {
        let text = text.trim();
        if self.busy || text.is_empty() {
            return None;
        }
        self.end_segment();
        self.messages.push(Message {
            role: Role::User,
            title: "你".into(),
            text: text.into(),
            updates: Vec::new(),
            compaction: None,
            tool_call: None,
            subagent: None,
        });
        self.busy = true;
        self.error = None;
        Some(text.into())
    }

    pub fn receive(&mut self, notification: SessionNotification) -> Option<usize> {
        if let SessionUpdate::ConfigOptionUpdate(update) = notification.update {
            self.set_config_options(update.config_options);
            return None;
        }
        if matches!(&notification.update, SessionUpdate::UsageUpdate(_)) {
            return None;
        }
        if let SessionUpdate::SessionMessage(_) = &notification.update {
            return self.receive_subagent_message(notification);
        }
        if let SessionUpdate::SessionMessageChunk(_) = &notification.update {
            return self.receive_subagent_chunk(notification);
        }
        if matches!(&notification.update, SessionUpdate::SubagentUpdate(_)) {
            return self.receive_subagent(notification);
        }
        if matches!(
            &notification.update,
            SessionUpdate::ToolCall(_) | SessionUpdate::ToolCallUpdate(_)
        ) {
            return Some(self.receive_tool_call(notification));
        }
        if let SessionUpdate::AvailableCommandsUpdate(update) = notification.update {
            self.available_commands = update.available_commands;
            return None;
        }
        if let SessionUpdate::SessionInfoUpdate(update) = &notification.update {
            update.title.clone().update_to(&mut self.session_title);
            return None;
        }
        if matches!(
            &notification.update,
            SessionUpdate::CompactionUpdate(_) | SessionUpdate::CompactionSummaryChunk(_)
        ) {
            self.end_segment();
            return Some(self.receive_compaction(notification));
        }
        let chunk = streamed_content(&notification.update);
        if let Some((role, title, chunk)) = chunk
            && (role == Role::User || matches!(chunk.content, ContentBlock::Text(_)))
        {
            let text = match &chunk.content {
                ContentBlock::Text(content) => content.text.as_str(),
                _ => "",
            };
            if let Some(index) = self.stream
                && index + 1 == self.messages.len()
                && self.messages[index].role == role
                && self.messages[index].updates.last().is_some_and(|previous| {
                    previous.session_id == notification.session_id
                        && streamed_content(&previous.update)
                            .is_some_and(|(_, _, previous)| previous.message_id == chunk.message_id)
                })
            {
                self.messages[index].text.push_str(text);
                self.messages[index].updates.push(notification);
                return Some(index);
            }
            self.stream = Some(self.messages.len());
            self.messages.push(Message {
                role,
                title: title.into(),
                text: text.into(),
                updates: vec![notification],
                compaction: None,
                tool_call: None,
                subagent: None,
            });
        } else {
            self.end_segment();
            let (title, text) = UpdateDisplay::from(&notification.update).into_parts();
            self.messages.push(Message {
                role: Role::Event,
                title,
                text,
                updates: vec![notification],
                compaction: None,
                tool_call: None,
                subagent: None,
            });
        }
        Some(self.messages.len() - 1)
    }

    fn ensure_subagent_message(&mut self, session_id: &SessionId) -> Option<usize> {
        if let Some(&index) = self.subagents.get(session_id) {
            return Some(index);
        }
        self.end_segment();
        let index = self.messages.len();
        self.messages.push(Message {
            role: Role::Event,
            title: "子 Agent".into(),
            text: String::new(),
            updates: Vec::new(),
            compaction: None,
            tool_call: None,
            subagent: Some(SubagentState::new(session_id.clone())),
        });
        self.subagents.insert(session_id.clone(), index);
        Some(index)
    }

    fn receive_subagent_message(&mut self, notification: SessionNotification) -> Option<usize> {
        let SessionUpdate::SessionMessage(msg) = &notification.update else {
            return None;
        };
        let parent_session_id = notification.session_id.clone();
        let subagent_session_id = match &msg.sender_session_id {
            MaybeUndefined::Value(id) if id != &parent_session_id => Some(id),
            _ => match &msg.recipient_session_id {
                MaybeUndefined::Value(id) if id != &parent_session_id => Some(id),
                _ => None,
            },
        };
        let subagent_session_id = subagent_session_id?;
        let route_key = (parent_session_id.clone(), msg.message_id.to_string());
        self.subagent_message_route.insert(route_key.clone(), subagent_session_id.clone());
        let index = self.ensure_subagent_message(subagent_session_id)?;
        let subagent = self.messages[index].subagent.as_mut()?;
        if let MaybeUndefined::Value(content) = &msg.content {
            let text = content.iter().fold(String::new(), |mut acc, block| {
                if let ContentBlock::Text(t) = block {
                    acc.push_str(&t.text);
                }
                acc
            });
            if !text.is_empty() {
                subagent.upsert_message(msg.message_id.to_string(), text);
            }
        }
        if let Some(pending) = self.pending_subagent_chunks.remove(&route_key) {
            for text in pending {
                subagent.append_chunk(msg.message_id.to_string(), &text);
            }
        }
        Some(index)
    }

    fn receive_subagent_chunk(&mut self, notification: SessionNotification) -> Option<usize> {
        let SessionUpdate::SessionMessageChunk(chunk) = &notification.update else {
            return None;
        };
        let text = match &chunk.content {
            ContentBlock::Text(t) => t.text.as_str(),
            _ => return None,
        };
        if text.is_empty() {
            return None;
        }
        let route_key = (notification.session_id.clone(), chunk.message_id.to_string());
        let Some(subagent_session_id) = self.subagent_message_route.get(&route_key).cloned() else {
            self.pending_subagent_chunks.entry(route_key).or_default().push(text.into());
            return None;
        };
        let index = self.ensure_subagent_message(&subagent_session_id)?;
        let subagent = self.messages[index].subagent.as_mut()?;
        subagent.append_chunk(chunk.message_id.to_string(), text);
        Some(index)
    }

    fn receive_subagent(&mut self, notification: SessionNotification) -> Option<usize> {
        let SessionUpdate::SubagentUpdate(update) = &notification.update else {
            return None;
        };
        let session_id = update.session_id.clone();
        let index = self.ensure_subagent_message(&session_id)?;
        let message = &mut self.messages[index];
        let subagent = message.subagent.as_mut()?;
        let SessionUpdate::SubagentUpdate(update) = notification.update else {
            return None;
        };
        subagent.update_metadata(update);
        message.title = subagent.title().unwrap_or("子 Agent").into();
        message.text = subagent.description().unwrap_or_default().into();
        Some(index)
    }

    fn receive_tool_call(&mut self, notification: SessionNotification) -> usize {
        let id = match &notification.update {
            SessionUpdate::ToolCall(call) => call.tool_call_id.clone(),
            SessionUpdate::ToolCallUpdate(update) => update.tool_call_id.clone(),
            _ => unreachable!(),
        };
        let key = (notification.session_id.clone(), id.clone());
        let index = match self.tools.get(&key) {
            Some(index) => *index,
            None => {
                self.end_segment();
                let index = self.messages.len();
                self.messages.push(Message {
                    role: Role::Event,
                    title: String::new(),
                    text: String::new(),
                    updates: Vec::new(),
                    compaction: None,
                    tool_call: Some(ToolCallState::new(ToolCall::new(id, ""))),
                    subagent: None,
                });
                self.tools.insert(key, index);
                index
            }
        };
        let message = &mut self.messages[index];
        let tool = message.tool_call.as_mut().expect("tool entry has state");
        match &notification.update {
            SessionUpdate::ToolCall(call) => tool.replace(call.clone()),
            SessionUpdate::ToolCallUpdate(update) => tool.update(update.fields.clone()),
            _ => unreachable!(),
        }
        message.title = tool.title().into();
        message.updates.clear();
        message.updates.push(notification);
        index
    }

    fn receive_compaction(&mut self, notification: SessionNotification) -> usize {
        let id = match &notification.update {
            SessionUpdate::CompactionUpdate(update) => &update.compaction_id,
            SessionUpdate::CompactionSummaryChunk(chunk) => &chunk.compaction_id,
            _ => unreachable!("only compaction notifications reach this handler"),
        };
        let index = self
            .messages
            .iter()
            .position(|message| {
                message.compaction.as_ref().is_some_and(|state| &state.compaction_id == id)
                    && message
                        .updates
                        .first()
                        .is_some_and(|first| first.session_id == notification.session_id)
            })
            .unwrap_or_else(|| {
                self.messages.push(Message {
                    role: Role::Event,
                    title: String::new(),
                    text: String::new(),
                    updates: Vec::new(),
                    compaction: Some(CompactionUpdate::new(
                        id.clone(),
                        CompactionStatus::InProgress,
                    )),
                    tool_call: None,
                    subagent: None,
                });
                self.messages.len() - 1
            });
        let message = &mut self.messages[index];
        let state = message.compaction.as_mut().expect("compaction entry has state");
        match &notification.update {
            SessionUpdate::CompactionUpdate(patch) => {
                state.status = patch.status.clone();
                if !patch.summary.is_undefined() {
                    state.summary = patch.summary.clone();
                }
                if !patch.error.is_undefined() {
                    state.error = patch.error.clone();
                }
                if !patch.meta.is_undefined() {
                    state.meta = patch.meta.clone();
                }
            }
            SessionUpdate::CompactionSummaryChunk(chunk) => {
                if let MaybeUndefined::Value(summary) = &mut state.summary {
                    summary.push(chunk.content.clone());
                } else {
                    state.summary = MaybeUndefined::Value(vec![chunk.content.clone()]);
                }
            }
            _ => unreachable!(),
        }
        (message.title, message.text) =
            UpdateDisplay::from(&SessionUpdate::CompactionUpdate(state.clone())).into_parts();
        message.updates.push(notification);
        index
    }

    pub fn end_segment(&mut self) {
        self.stream = None;
    }

    pub fn finish(&mut self, error: Option<String>) {
        self.end_segment();
        for message in &mut self.messages {
            if let Some(tool) = &mut message.tool_call {
                tool.finish();
            }
        }
        self.busy = false;
        self.error = error;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_title(&mut self, title: Option<String>) {
        self.session_title = title;
    }

    pub fn title(&self) -> String {
        if let Some(title) = self.session_title.as_deref().filter(|title| !title.trim().is_empty())
        {
            return title.to_owned();
        }
        self.messages
            .iter()
            .find(|message| message.role == Role::User && !message.text.trim().is_empty())
            .map(|message| {
                let line = message.text.lines().next().unwrap_or_default();
                let mut title: String = line.chars().take(24).collect();
                if line.chars().count() > 24 {
                    title.push('…');
                }
                title
            })
            .unwrap_or_else(|| "新对话".into())
    }
}

fn streamed_content(update: &SessionUpdate) -> Option<(Role, &'static str, &ContentChunk)> {
    match update {
        SessionUpdate::AgentMessageChunk(chunk) => Some((Role::Assistant, "", chunk)),
        SessionUpdate::AgentThoughtChunk(chunk) => Some((Role::Thought, "思考", chunk)),
        SessionUpdate::UserMessageChunk(chunk) => Some((Role::User, "用户消息", chunk)),
        _ => None,
    }
}
