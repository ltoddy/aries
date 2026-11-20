use crate::workspace::session::execution::*;

impl Execution {
    pub fn apply_event(
        &mut self,
        ui: &mut SessionParts<'_>,
        event: Event,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let mut sidebar_synced = false;
        match event {
            Event::Connected { list_sessions, load_session, prompt_capabilities, .. } => {
                ui.sidebar.list_mut().connected(list_sessions);
                self.load_supported = load_session;
                self.prompt_capabilities = prompt_capabilities;
                if let Some(session) = self.queued_session_load.take() {
                    self.load_session(ui, session, cx);
                }
                if ui.render_mode != crate::workspace::SessionRenderMode::Main {
                    self.refresh_sessions(ui, cx);
                }
            },
            Event::Sessions(result) => {
                let received = ui.sidebar.list().accepts_response() && result.is_ok();
                ui.sidebar.list_mut().receive(result);
                if received {
                    ui.sidebar.receive_sidebar_rows(
                        self.session_id.as_ref(),
                        &self.cwd,
                        self.conversation.title(),
                        cx,
                    );
                    sidebar_synced = true;
                }
                if ui.sidebar.list_mut().take_refresh_request() {
                    self.refresh_sessions(ui, cx);
                }
            },
            Event::SessionTitleSet { .. }
            | Event::SessionArchived { .. }
            | Event::SessionForked { .. } => {},
            Event::SessionsDeleted(result) => self.finish_deletion(ui, result, window, cx),
            Event::SessionLoaded { session_id, result } => {
                self.finish_session_load(ui, &session_id, result, window, cx);
            },
            Event::SessionConfigSet { session_id, config_id, result } => {
                ui.configuration.finish_session_config(self, &session_id, &config_id, result, cx);
            },
            Event::Ready(response) => {
                let creating_session = std::mem::take(&mut self.creating_session);
                self.session_id = Some(response.session_id.clone());
                if creating_session {
                    let session = agent_client_protocol::v1::schema::SessionInfo::new(
                        response.session_id,
                        self.cwd.clone(),
                    )
                    .title(self.conversation.title());
                    if !ui.sidebar.list().show_archived() {
                        ui.sidebar.motion_mut().add_created(session.clone());
                    }
                    ui.sidebar.set_created_session(session);
                }
                self.conversation.end_segment();
                for notification in std::mem::take(&mut self.pending_updates) {
                    self.receive_update(ui, notification);
                }
                if let Some(options) = response.config_options {
                    self.conversation.set_config_options(options);
                }
                if creating_session && ui.render_mode != crate::workspace::SessionRenderMode::Main {
                    self.refresh_sessions(ui, cx);
                }
            },
            Event::Update(notification) => {
                if let SessionUpdate::SessionInfoUpdate(update) = &notification.update {
                    ui.sidebar.list_mut().update(&notification.session_id, update);
                    ui.sidebar.sync_sidebar_rows(
                        self.session_id.as_ref(),
                        &self.cwd,
                        self.conversation.title(),
                        cx,
                    );
                    if ui.render_mode != crate::workspace::SessionRenderMode::Main {
                        self.refresh_sessions(ui, cx);
                    }
                }
                if let Some(loading) = &mut self.loading_session
                    && loading.session_id() == &notification.session_id
                {
                    loading.receive(*notification);
                    cx.notify();
                    return;
                }
                if self.session_id.is_none()
                    && (self.conversation.is_busy() || self.creating_session)
                {
                    self.pending_updates.push(*notification);
                } else {
                    self.receive_update(ui, *notification);
                }
            },
            Event::Permission(permission) => {
                if self.stopping {
                    permission.respond(None);
                } else {
                    self.permissions.push_back(permission);
                }
            },
            Event::Finished(result) => {
                self.dismiss_permissions();
                let automatic = matches!(&result, Ok(StopReason::EndTurn)) && !self.stopping;
                self.stopping = false;
                self.conversation.finish(result.err().map(|error| {
                    crate::error::log("Agent 执行失败", &error);
                    crate::error::details(&error)
                }));
                if automatic && self.prompt_queue().is_some_and(PromptQueue::is_automatic) {
                    self.send_prompt_queue(ui, window, cx);
                } else {
                    self.with_prompt_queue(PromptQueue::pause);
                }
                if ui.render_mode != crate::workspace::SessionRenderMode::Main {
                    self.refresh_sessions(ui, cx);
                }
            },
            Event::Disconnected(error) => {
                self.dismiss_permissions();
                self.with_prompt_queue(PromptQueue::pause);
                self.stopping = false;
                self.disconnected = true;
                self.load_supported = false;
                self.loading_session = None;
                self.creating_session = false;
                self.pending_updates.clear();
                ui.configuration.disconnect();
                ui.sidebar.list_mut().disconnect(error.clone());
                ui.deletion.disconnect();
                self.conversation.finish(Some(format!("{error}\n请新建对话重新连接 Aries。")));
            },
        }
        if !sidebar_synced && ui.render_mode != crate::workspace::SessionRenderMode::Main {
            ui.sidebar.sync_sidebar_rows(
                self.session_id.as_ref(),
                &self.cwd,
                self.conversation.title(),
                cx,
            );
        }
        cx.notify();
    }

    fn receive_update(&mut self, ui: &mut SessionParts<'_>, notification: SessionNotification) {
        if self.session_id.as_ref() != Some(&notification.session_id) {
            return;
        }
        if let SessionUpdate::UsageUpdate(usage) = &notification.update {
            self.tokens = Some(usage.used);
        }
        if matches!(&notification.update, SessionUpdate::AvailableCommandsUpdate(_)) {
            ui.composer.reset_menu();
        }
        if let Some(index) = self.conversation.receive(notification) {
            if let Some(subagent) = self.conversation.messages()[index].subagent() {
                ui.timeline.sync_subagent(subagent);
            }
            if index < ui.timeline.list().item_count() {
                ui.timeline.list().remeasure_items(index..index + 1);
            }
            ui.timeline.cache_message_images(index, &self.conversation);
            ui.timeline.cache_tool_diffs(index, &self.conversation);
        }
    }
}
