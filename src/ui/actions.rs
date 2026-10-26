use gpui_kit::*;

use crate::acp::{Command, Event, PromptTarget};
use crate::ui::ChatView;
use crate::ui::new_chat::NewChatAction;

impl ChatView {
    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_send(cx) {
            return;
        }
        if self.session_id.is_none() && !self.session_directory_selected {
            self.choose_session_directory(NewChatAction::SendPrompt, window, cx);
            return;
        }
        let Some(prompt) = self.chat.begin_turn(&self.input.read(cx).value()) else {
            return;
        };
        if self.bridge.is_none() {
            self.connect(window, cx);
        }
        if !self.session_list.is_connecting() && !self.disconnected {
            self.status = "思考中…".into();
        }
        let target = self
            .session_id
            .clone()
            .map(PromptTarget::Existing)
            .unwrap_or_else(|| PromptTarget::New(self.cwd.clone()));
        if self
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.send(Command::Prompt { target, text: prompt }).is_ok())
        {
            self.input.update(cx, |input, cx| input.set_value("", window, cx));
        } else {
            self.apply_event(Event::Disconnected("无法发送消息，请新建对话".into()), window, cx);
        }
        self.timeline.scroll_to_end();
        cx.notify();
    }

    pub fn is_session_idle(&self) -> bool {
        !self.chat.is_busy()
            && !self.session_deletion.is_busy()
            && self.loading_session.is_none()
            && self.creating_session.is_none()
            && !self.choosing_directory
            && self.pending_config.is_none()
    }

    pub fn can_send(&self, cx: &App) -> bool {
        self.is_session_idle()
            && !self.disconnected
            && !self.input.read(cx).value().trim().is_empty()
    }

    pub fn save_draft(&mut self, cx: &App) {
        if let Some(id) = &self.session_id {
            let draft = self.input.read(cx).value().to_string();
            if draft.is_empty() {
                self.drafts.remove(id);
            } else {
                self.drafts.insert(id.clone(), draft);
            }
        }
    }

    pub fn stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.chat.is_busy() || self.stopping {
            return;
        }
        self.dismiss_permissions();
        if self.bridge.as_ref().is_some_and(|bridge| bridge.send(Command::Cancel).is_ok()) {
            self.stopping = true;
            self.status = "正在停止…".into();
        } else {
            self.apply_event(Event::Disconnected("连接已关闭，请新建对话".into()), window, cx);
        }
        cx.notify();
    }

    pub fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.choose_session_directory(NewChatAction::Create, window, cx);
    }

    pub fn reset_chat(&mut self) {
        self.chat.reset();
        self.command_menu.reset();
        self.expanded_thoughts.clear();
        self.collapsed_streaming_thoughts.clear();
        self.tool_diffs.clear();
        self.tool_diff_rows.clear();
        self.images.clear();
        self.attachment_images.clear();
        self.markdown.clear();
        self.session_id = None;
        self.queued_session_load = None;
        self.pending_updates.clear();
        self.session_directory_selected = false;
        self.loading_session = None;
        self.creating_session = None;
        self.stopping = false;
        self.status = "就绪".into();
        self.tokens = None;
        self.pending_config = None;
        self.config_error = None;
        self.timeline.reset(0);
    }
}
