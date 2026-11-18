use crate::workspace::session::session_view::*;

impl SessionView {
    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.send(parts, window, cx));
    }
    pub fn toggle_prompt_queue(&mut self, cx: &mut Context<Self>) {
        self.execution.with_prompt_queue(PromptQueue::toggle);
        cx.notify();
    }
    pub fn remove_queued_prompt(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        self.execution.with_prompt_queue(|queue| queue.remove(id));
        self.input().update(cx, |input, cx| input.focus(window, cx));
        cx.notify();
    }
    pub fn stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.stop(parts, window, cx));
    }
    pub fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.connect(parts, window, cx));
    }
    pub fn apply_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        if self.render_mode == SessionRenderMode::Main && !self.management.has_local_pending() {
            if matches!(
                &event,
                Event::SessionTitleSet { .. }
                    | Event::SessionArchived { .. }
                    | Event::SessionForked { .. }
            ) {
                if let Some(host) = self.tab_host() {
                    window.defer(cx, move |window, cx| {
                        let _ = host.update(cx, |host, cx| {
                            host.forward_session_management_event(event, window, cx)
                        });
                    });
                    return;
                }
            } else if let Event::Update(notification) = &event
                && matches!(
                    &notification.update,
                    agent_client_protocol::v1::schema::SessionUpdate::SessionInfoUpdate(_)
                )
                && let Some(host) = self.tab_host()
            {
                let notification = notification.clone();
                window.defer(cx, move |window, cx| {
                    let _ = host.update(cx, |host, cx| {
                        host.forward_session_management_event(
                            Event::Update(notification),
                            window,
                            cx,
                        )
                    });
                });
            } else if matches!(&event, Event::Disconnected(_))
                && self.management.is_locked()
                && let Some(id) = self.active_session_id().cloned()
                && let Some(host) = self.tab_host()
            {
                cx.defer(move |cx| {
                    let _ = host.update(cx, |host, cx| host.fail_session_management(&id, cx));
                });
            }
        }
        if !self.apply_management_event(&event, window, cx) {
            self.coordinate(|execution, parts| execution.apply_event(parts, event, window, cx));
        }
    }
    pub fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.new_chat(parts, window, cx));
    }
    pub fn new_chat_in_directory(
        &mut self,
        directory: impl Into<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.coordinate(|execution, parts| {
            execution.new_chat_in_directory(parts, directory, window, cx)
        });
    }
    pub fn new_chat_in_current_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.new_chat_in_directory(self.execution.directory().to_owned(), window, cx);
    }
    pub fn start_new_chat(
        &mut self,
        directory: impl Into<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.coordinate(|execution, parts| execution.start_new_chat(parts, directory, window, cx));
    }
    pub fn finish_directory_selection(&mut self) {
        self.execution.finish_directory_selection();
    }
    pub fn load_session(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.load_session(parts, session, cx));
    }
    pub fn queue_session_load(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.queue_session_load(parts, session, cx));
    }
    pub fn refresh_sessions(&mut self, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.refresh_sessions(parts, cx));
    }
    pub fn choose_permission(&mut self, option: PermissionOptionId, cx: &mut Context<Self>) {
        self.execution.choose_permission(option, cx);
    }
    pub fn set_session_config(
        &mut self,
        request: SetSessionConfigOptionRequest,
        cx: &mut Context<Self>,
    ) {
        let idle = self.is_session_idle();
        self.configuration.set_session_config(&self.execution, idle, request, cx);
    }
    pub fn command_menu_action(
        &mut self,
        action: CommandMenuAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let available = self.commands_available();
        self.composer.command_menu_action(
            action,
            self.execution.conversation().available_commands(),
            available,
            window,
            cx,
        );
    }
    pub fn select_command(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.composer.select_command(name, window, cx);
    }
    pub fn jump_user_message(&mut self, next: bool, cx: &mut Context<Self>) {
        self.timeline.jump_user_message(self.execution.conversation(), next, cx);
    }
    pub fn remeasure_timeline(&self) {
        self.timeline.list().remeasure();
    }
}
