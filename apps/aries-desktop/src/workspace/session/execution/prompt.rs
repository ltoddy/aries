use crate::workspace::session::execution::*;

impl Execution {
    pub fn send(
        &mut self,
        ui: &mut SessionParts<'_>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if !self.can_send(ui, cx) {
            return;
        }
        let Some(session_id) = self.session_id.clone() else {
            return;
        };
        if self.conversation.is_busy() || self.prompt_queue().is_some() {
            if !self
                .prompt_queues
                .entry(session_id.clone())
                .or_default()
                .push(ui.composer.draft(cx))
            {
                return;
            }
            ui.composer.clear(window, cx);
            self.drafts.remove(&session_id);
            if !self.conversation.is_busy() {
                self.send_prompt_queue(ui, window, cx);
            }
        } else if self.send_content(ui, session_id.clone(), ui.composer.content(cx), window, cx) {
            ui.composer.clear(window, cx);
            self.drafts.remove(&session_id);
        }
        cx.notify();
    }

    fn send_content(
        &mut self,
        ui: &mut SessionParts<'_>,
        session_id: SessionId,
        content: Vec<ContentBlock>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) -> bool {
        if !self.conversation.begin_turn(&session_id, &content) {
            return false;
        }
        if self.bridge.is_none() {
            self.connect(ui, window, cx);
        }
        let sent = self
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.send(Command::Prompt { session_id, content }).is_ok());
        if !sent {
            self.apply_event(
                ui,
                Event::Disconnected("无法发送消息，请新建对话".into()),
                window,
                cx,
            );
        }
        ui.timeline.list().set_follow_mode(FollowMode::Tail);
        cx.notify();
        sent
    }

    pub fn prompt_queue(&self) -> Option<&PromptQueue> {
        self.session_id.as_ref().and_then(|id| self.prompt_queues.get(id))
    }

    pub fn is_prompt_queue_full(&self) -> bool {
        self.prompt_queue().is_some_and(PromptQueue::is_full)
    }

    pub fn with_prompt_queue(&mut self, operation: impl FnOnce(&mut PromptQueue)) {
        if let Some(id) = &self.session_id
            && let Some(queue) = self.prompt_queues.get_mut(id)
        {
            operation(queue);
            if queue.is_empty() {
                self.prompt_queues.remove(id);
            }
        }
    }

    pub fn send_prompt_queue(
        &mut self,
        ui: &mut SessionParts<'_>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) -> bool {
        if self.conversation.is_busy()
            || !self.is_prompt_available(
                (ui.deletion.is_busy() || ui.management_pending) || ui.configuration.is_pending(),
            )
        {
            return false;
        }
        let Some(id) = self.session_id.clone() else {
            return false;
        };
        let Some((prompt_id, content)) = self.prompt_queue().and_then(PromptQueue::next_prompt)
        else {
            return false;
        };
        let sent = self.send_content(ui, id, content, window, cx);
        if sent {
            self.with_prompt_queue(|queue| {
                queue.remove(prompt_id);
                queue.resume();
            });
        }
        cx.notify();
        sent
    }

    pub fn is_prompt_available(&self, blocked: bool) -> bool {
        self.session_id.is_some()
            && !blocked
            && !self.disconnected
            && !self.stopping
            && self.loading_session.is_none()
            && !self.creating_session
            && !self.choosing_directory
    }

    pub fn is_idle(&self, blocked: bool) -> bool {
        !blocked
            && !self.conversation.is_busy()
            && self.loading_session.is_none()
            && !self.creating_session
            && !self.choosing_directory
    }
    pub fn is_session_idle(&self, ui: &SessionParts<'_>) -> bool {
        self.is_idle(
            (ui.deletion.is_busy() || ui.management_pending)
                || ui.configuration.is_pending()
                || ui.composer.is_loading(),
        )
    }
    pub fn can_send(&self, ui: &SessionParts<'_>, cx: &App) -> bool {
        self.is_prompt_available(
            (ui.deletion.is_busy() || ui.management_pending)
                || ui.configuration.is_pending()
                || ui.composer.is_loading(),
        ) && !self.is_prompt_queue_full()
            && ui.composer.has_content(cx)
    }
    pub fn supports_load(&self) -> bool {
        self.load_supported
    }
    pub fn save_draft(&mut self, ui: &SessionParts<'_>, cx: &App) {
        if let Some(id) = &self.session_id {
            let draft = ui.composer.draft(cx);
            if draft.is_empty() {
                self.drafts.remove(id);
            } else {
                self.drafts.insert(id.clone(), draft);
            }
        }
    }

    pub fn stop(
        &mut self,
        ui: &mut SessionParts<'_>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if !self.conversation.is_busy() || self.stopping {
            return;
        }
        self.dismiss_permissions();
        self.with_prompt_queue(PromptQueue::pause);
        if self.bridge.as_ref().is_some_and(|bridge| bridge.send(Command::Cancel).is_ok()) {
            self.stopping = true;
        } else {
            self.apply_event(ui, Event::Disconnected("连接已关闭，请新建对话".into()), window, cx);
        }
        cx.notify();
    }
}
