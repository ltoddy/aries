use agent_client_protocol::schema::v1::{
    PermissionOptionId, SessionNotification, SessionUpdate, StopReason,
};
use gpui_kit::*;

use crate::acp::{Bridge, Event};
use crate::ui::ChatView;
use crate::ui::new_chat::NewChatAction;

impl ChatView {
    pub fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.bridge.is_some() {
            return;
        }
        self.disconnected = false;
        self.load_supported = false;
        self.session_list.connect();
        self.status = "正在连接 Aries…".into();
        match Bridge::start(self.agent.clone()) {
            Ok(bridge) => {
                let events = bridge.events().clone();
                self.receiver = Some(cx.spawn_in(window, async move |view, cx| {
                    while let Ok(event) = events.recv().await {
                        if view
                            .update_in(cx, |this, window, cx| this.apply_event(event, window, cx))
                            .is_err()
                        {
                            return;
                        }
                    }
                    let _ = view.update_in(cx, |this, window, cx| {
                        if !this.disconnected {
                            this.apply_event(
                                Event::Disconnected("Agent 连接已结束，请新建对话".into()),
                                window,
                                cx,
                            );
                        }
                    });
                }));
                self.bridge = Some(bridge);
            }
            Err(error) => {
                self.apply_event(
                    Event::Disconnected(format!("无法启动 Agent：{error}")),
                    window,
                    cx,
                );
            }
        }
        cx.notify();
    }

    pub fn apply_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        let follow = self.timeline.is_scrolled_to_end().unwrap_or(true);
        match event {
            Event::Connected { list_sessions, load_session } => {
                self.session_list.connected(list_sessions);
                self.load_supported = load_session;
                if !self.stopping && self.creating_session.is_none() {
                    self.status = if self.chat.is_busy() { "思考中…" } else { "就绪" }.into();
                }
                if let Some(session) = self.queued_session_load.take() {
                    self.load_session(session, cx);
                }
                if self.render_mode != crate::ui::ChatRenderMode::Main {
                    self.refresh_sessions(cx);
                }
            }
            Event::Sessions(result) => {
                self.session_list.receive(result);
                if self.session_list.take_refresh_request() {
                    self.refresh_sessions(cx);
                }
            }
            Event::SessionsDeleted(result) => self.apply_session_deletion(result, window, cx),
            Event::SessionLoaded { session_id, result } => {
                self.finish_session_load(session_id, result, window, cx);
            }
            Event::SessionConfigSet { session_id, config_id, result } => {
                self.finish_session_config(session_id, config_id, result, cx);
            }
            Event::Ready(response) => {
                let action = self.creating_session.take();
                self.session_id = Some(response.session_id);
                self.chat.end_segment();
                self.status = if self.chat.is_busy() { "思考中…" } else { "就绪" }.into();
                for notification in std::mem::take(&mut self.pending_updates) {
                    self.receive_update(notification, cx);
                }
                if let Some(options) = response.config_options {
                    self.chat.set_config_options(options);
                }
                if action.is_some() && self.render_mode != crate::ui::ChatRenderMode::Main {
                    self.refresh_sessions(cx);
                }
                if matches!(action, Some(NewChatAction::SendPrompt)) {
                    self.send(window, cx);
                }
            }
            Event::Update(notification) => {
                if let SessionUpdate::SessionInfoUpdate(update) = &notification.update {
                    self.session_list.update(&notification.session_id, update);
                }
                if let Some(loading) = &mut self.loading_session
                    && loading.session_id() == &notification.session_id
                {
                    loading.receive(*notification);
                    cx.notify();
                    return;
                }
                if self.session_id.is_none()
                    && (self.chat.is_busy() || self.creating_session.is_some())
                {
                    self.pending_updates.push(*notification);
                } else {
                    self.receive_update(*notification, cx);
                }
            }
            Event::Permission(permission) => {
                if self.stopping {
                    permission.respond(None);
                } else {
                    self.permissions.push_back(permission);
                    self.status = "等待你的选择".into();
                }
            }
            Event::Finished(result) => {
                self.dismiss_permissions();
                self.stopping = false;
                self.status = match &result {
                    Ok(StopReason::Cancelled) => "已停止",
                    Ok(_) => "就绪",
                    Err(_) => "回复失败",
                }
                .into();
                self.chat.finish(result.err().map(|error| error.to_string()));
                if self.render_mode != crate::ui::ChatRenderMode::Main {
                    self.refresh_sessions(cx);
                }
            }
            Event::Disconnected(error) => {
                self.dismiss_permissions();
                self.stopping = false;
                self.disconnected = true;
                self.load_supported = false;
                self.loading_session = None;
                self.creating_session = None;
                self.pending_updates.clear();
                if self.pending_config.take().is_some() {
                    self.config_error = Some("连接已断开，无法确认配置是否更新。".into());
                }
                self.session_list.disconnect(error.clone());
                self.session_deletion.disconnect();
                self.status = "连接已断开".into();
                self.chat.finish(Some(format!("{error}\n请新建对话重新连接 Aries。")));
            }
        }
        if follow {
            self.timeline.scroll_to_end();
        }
        cx.notify();
    }

    fn receive_update(&mut self, notification: SessionNotification, cx: &mut Context<Self>) {
        if self.session_id.as_ref() != Some(&notification.session_id) {
            return;
        }
        if let SessionUpdate::UsageUpdate(usage) = &notification.update {
            self.tokens = Some(usage.used);
        }
        if self.chat.is_busy() && !self.stopping && self.permissions.is_empty() {
            match &notification.update {
                SessionUpdate::AgentThoughtChunk(_) => self.status = "思考中…".into(),
                SessionUpdate::AgentMessageChunk(_) => self.status = "正在回复…".into(),
                SessionUpdate::ToolCall(tool) => self.status = tool.title.clone(),
                _ => {}
            }
        }
        if matches!(&notification.update, SessionUpdate::AvailableCommandsUpdate(_)) {
            self.command_menu.reset();
        }
        if let Some(index) = self.chat.receive(notification) {
            self.cache_message_images(index);
            self.cache_tool_diffs(index, cx);
        }
    }

    pub fn choose_permission(&mut self, option: PermissionOptionId, cx: &mut Context<Self>) {
        if let Some(permission) = self.permissions.pop_front() {
            permission.respond(Some(option));
        }
        self.status =
            if self.permissions.is_empty() { "正在执行…" } else { "等待你的选择" }.into();
        cx.notify();
    }

    pub fn dismiss_permissions(&mut self) {
        for permission in self.permissions.drain(..) {
            permission.respond(None);
        }
    }
}
