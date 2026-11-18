use std::collections::{HashMap, HashSet};
use std::time::Duration;

use agent_client_protocol::schema::MaybeUndefined;
use agent_client_protocol::v1::schema::{CompactionStatus, SessionId, SessionUpdate};
use gpui_kit::assets::IconName;
use gpui_kit::base::SelectableText;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::text::{TextView, TextViewState};
use gpui_kit::component::{ActiveTheme, Disableable, Icon, Sizable};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::workspace::SessionView;
use crate::workspace::conversation::attachment::{Attachment, resource_uri};
use crate::workspace::conversation::media::MessageImages;
use crate::workspace::conversation::model::display::UpdateDisplay;
use crate::workspace::conversation::model::subagent::SubagentState;
use crate::workspace::conversation::model::{Conversation, Role};
use crate::workspace::conversation::subagent_message::SubagentMessage;
use crate::workspace::conversation::text::link_handler;
use crate::workspace::conversation::thought_message::ThoughtMessage;
use crate::workspace::conversation::tool_content::ToolDiffs;
use crate::workspace::conversation::tool_message::{MessageStatusIcon, ToolMessage};
use crate::workspace::conversation::user_message::UserMessage;
use crate::workspace::settings::palette::Palette;
use crate::workspace::settings::typography;
use crate::{NextUserMessage, PreviousUserMessage};

fn user_message_target(
    anchors: impl DoubleEndedIterator<Item = usize>,
    current: usize,
    inside: bool,
    next: bool,
) -> Option<usize> {
    if next {
        anchors.into_iter().find(|index| *index > current)
    } else {
        anchors.into_iter().rev().find(|index| *index < current || (*index == current && inside))
    }
}

pub struct Timeline {
    list: ListState,
    markdown: HashMap<usize, Entity<TextViewState>>,
    expanded_thoughts: HashSet<usize>,
    collapsed_streaming_thoughts: HashSet<usize>,
    expanded_subagents: HashSet<SessionId>,
    idle_subagents: HashSet<SessionId>,
    images: MessageImages,
    tool_diffs: ToolDiffs,
}

impl Timeline {
    pub fn new(cx: &mut Context<SessionView>) -> Self {
        let list = ListState::new(0, ListAlignment::Bottom, px(600.));
        list.set_follow_mode(FollowMode::Tail);
        let view = cx.weak_entity();
        list.set_scroll_handler(move |_, _, cx| {
            let _ = view.update(cx, |_, cx| cx.notify());
        });
        Self {
            list,
            markdown: HashMap::new(),
            expanded_thoughts: HashSet::new(),
            collapsed_streaming_thoughts: HashSet::new(),
            expanded_subagents: HashSet::new(),
            idle_subagents: HashSet::new(),
            images: MessageImages::new(),
            tool_diffs: ToolDiffs::new(),
        }
    }
    pub fn list(&self) -> &ListState {
        &self.list
    }
    pub fn reset(&mut self) {
        self.markdown.clear();
        self.expanded_thoughts.clear();
        self.collapsed_streaming_thoughts.clear();
        self.expanded_subagents.clear();
        self.idle_subagents.clear();
        self.tool_diffs.clear();
        self.images.rebuild_images();
        self.list.reset(0);
        self.list.set_follow_mode(FollowMode::Tail);
    }
    pub fn cache_message_images(&mut self, index: usize, conversation: &Conversation) {
        self.images.cache_message_images(index, &conversation.messages()[index]);
    }
    pub fn rebuild_images(&mut self) {
        self.images.rebuild_images();
    }
    pub fn cache_tool_diffs(&mut self, index: usize, conversation: &Conversation) {
        self.tool_diffs.cache_tool_diffs(index, conversation);
    }
    pub fn rebuild_tool_diffs(&mut self, conversation: &Conversation) {
        self.tool_diffs.rebuild_tool_diffs(conversation);
    }
    pub fn toggle_thought(&mut self, index: usize, conversation: &Conversation) {
        let toggled = if conversation.is_streaming(index) {
            &mut self.collapsed_streaming_thoughts
        } else {
            &mut self.expanded_thoughts
        };
        if !toggled.remove(&index) {
            toggled.insert(index);
        }
    }
    pub fn sync_subagent(&mut self, subagent: &SubagentState) {
        let id = subagent.session_id();
        if subagent.is_idle() {
            if self.idle_subagents.insert(id.clone()) {
                self.expanded_subagents.remove(id);
            }
        } else {
            self.idle_subagents.remove(id);
        }
    }

    pub fn toggle_subagent(&mut self, id: &SessionId) {
        if !self.expanded_subagents.remove(id) {
            self.expanded_subagents.insert(id.clone());
        }
    }

    fn user_message_target(&self, conversation: &Conversation, next: bool) -> Option<usize> {
        let offset = self.list.logical_scroll_top();
        let anchors = conversation
            .messages()
            .iter()
            .enumerate()
            .filter_map(|(index, message)| (message.role() == Role::User).then_some(index));
        user_message_target(anchors, offset.item_ix, offset.offset_in_item > px(0.), next)
    }

    pub fn jump_user_message(
        &mut self,
        conversation: &Conversation,
        next: bool,
        cx: &mut Context<SessionView>,
    ) {
        if let Some(item_ix) = self.user_message_target(conversation, next) {
            self.list.scroll_to(ListOffset { item_ix, offset_in_item: px(0.) });
            cx.notify();
        }
    }
}

mod items;
mod render;

impl Timeline {
    fn sync_timeline_items(&self, conversation: &Conversation) {
        let count = conversation.messages().len() + usize::from(conversation.error().is_some());
        let current = self.list.item_count();
        if current < count {
            self.list.splice(current..current, count - current);
        } else if current > count {
            self.list.splice(count..current, 0);
        }
    }
}
