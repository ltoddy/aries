use crate::workspace::session::execution::*;

impl Execution {
    pub fn new_chat(
        &mut self,
        ui: &mut SessionParts<'_>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        self.choose_session_directory(ui, window, cx);
    }

    pub fn reset_chat(&mut self, ui: &mut SessionParts<'_>) {
        self.conversation.reset();
        ui.composer.reset_menu();
        ui.composer.clear_attachments();
        ui.timeline.reset();
        self.session_id = None;
        self.queued_session_load = None;
        self.pending_updates.clear();
        self.session_directory_selected = false;
        self.loading_session = None;
        self.creating_session = false;
        self.stopping = false;
        self.tokens = None;
        ui.configuration.reset();
        ui.timeline.list().reset(0);
        ui.timeline.list().set_follow_mode(FollowMode::Tail);
    }

    pub fn connect(
        &mut self,
        ui: &mut SessionParts<'_>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if self.bridge.is_some() {
            return;
        }
        self.disconnected = false;
        self.load_supported = false;
        self.prompt_capabilities = PromptCapabilities::new();
        ui.sidebar.list_mut().connect();
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
                        if !this.execution().is_disconnected() {
                            this.apply_event(
                                Event::Disconnected("Agent 连接已结束，请新建对话".into()),
                                window,
                                cx,
                            );
                        }
                    });
                }));
                self.bridge = Some(bridge);
            },
            Err(error) => {
                crate::error::report("无法启动 Agent", &error, window, cx);
                self.apply_event(
                    ui,
                    Event::Disconnected(format!(
                        "无法启动 Agent：{}",
                        crate::error::details(&error)
                    )),
                    window,
                    cx,
                );
            },
        }
        cx.notify();
    }

    pub fn can_load_sessions(&self, ui: &SessionParts<'_>) -> bool {
        self.load_supported
            && !self.disconnected
            && self.is_session_idle(ui)
            && !ui.sidebar.list().is_loading()
    }

    pub fn load_session(
        &mut self,
        ui: &mut SessionParts<'_>,
        session: SessionInfo,
        cx: &mut Context<SessionView>,
    ) {
        if !self.can_load_sessions(ui) || self.session_id.as_ref() == Some(&session.session_id) {
            return;
        }
        let request = LoadSessionRequest::new(session.session_id.clone(), session.cwd.clone());
        if !self
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.send(Command::LoadSession(Box::new(request))).is_ok())
        {
            self.conversation.finish(Some("无法发送加载请求，请重新连接后重试。".into()));
            cx.notify();
            return;
        }
        self.dismiss_permissions();
        self.conversation.finish(None);
        self.loading_session = Some(SessionLoad::new(session));
        cx.notify();
    }

    pub fn finish_session_load(
        &mut self,
        ui: &mut SessionParts<'_>,
        id: &SessionId,
        result: Result<LoadSessionResponse, Error>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if !self.loading_session.as_ref().is_some_and(|loading| loading.session_id() == id) {
            return;
        }
        let Some(loading) = self.loading_session.take() else {
            return;
        };
        self.dismiss_permissions();
        match result {
            Ok(response) => {
                let (session, mut conversation, tokens) = loading.into_parts();
                conversation.finish(None);
                if let Some(options) = response.config_options {
                    conversation.set_config_options(options);
                }
                self.save_draft(ui, cx);
                let draft = self.drafts.remove(&session.session_id).unwrap_or_default();
                self.reset_chat(ui);
                self.conversation = conversation;
                self.cwd = session.cwd;
                ui.git_panel.set_directory(self.cwd.clone());
                ui.git_panel.watch_info(cx);
                self.session_id = Some(session.session_id);
                self.tokens = tokens;
                ui.composer.restore_draft(draft, window, cx);
                ui.timeline.rebuild_images();
                ui.timeline.rebuild_tool_diffs(&self.conversation);
                for message in self.conversation.messages() {
                    if let Some(subagent) = message.subagent() {
                        ui.timeline.sync_subagent(subagent);
                    }
                }
                ui.timeline
                    .list()
                    .reset_with_uniform_height(self.conversation.messages().len(), px(96.));
                ui.timeline.list().scroll_to_end();
            },
            Err(error) => {
                self.conversation
                    .finish(Some(format!("无法加载会话：{}", crate::error::details(&error))));
                crate::error::report("无法加载会话", &error, window, cx);
            },
        }
        if ui.sidebar.list_mut().take_refresh_request() {
            self.refresh_sessions(ui, cx);
        }
        cx.notify();
    }

    pub fn can_start_new_chat(&self, ui: &SessionParts<'_>) -> bool {
        self.is_session_idle(ui)
    }

    pub fn choose_session_directory(
        &mut self,
        ui: &mut SessionParts<'_>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if !self.can_start_new_chat(ui) {
            return;
        }
        self.choosing_directory = true;
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择新会话的项目目录".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            let result: Result<Option<PathBuf>, SelectDirectoryError> = async {
                let Some(path) = selection
                    .await
                    .map_err(|source| SelectDirectoryError::wait(source.into()))?
                    .map_err(|source| SelectDirectoryError::open(source.into()))?
                    .and_then(|paths| paths.into_iter().next())
                else {
                    return Ok(None);
                };
                let directory = cx
                    .background_executor()
                    .spawn(async move { validate_directory(&path) })
                    .await
                    .map_err(SelectDirectoryError::validate)?;
                Ok(Some(directory))
            }
            .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.finish_directory_selection();
                match result {
                    Ok(Some(directory)) => {
                        if let Some(host) = this.tab_host().clone() {
                            window.defer(cx, move |window, cx| {
                                let _ = host.update(cx, |host, cx| {
                                    host.new_tab_in_directory(directory, window, cx);
                                });
                            });
                        } else {
                            this.start_new_chat(directory, window, cx);
                        }
                    },
                    Ok(None) => {},
                    Err(error) => {
                        crate::error::report("无法选择项目目录", &error, window, cx);
                    },
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn new_chat_in_directory(
        &mut self,
        ui: &mut SessionParts<'_>,
        directory: impl Into<PathBuf>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if !self.can_start_new_chat(ui) {
            return;
        }
        let directory = directory.into();
        self.choosing_directory = true;
        cx.spawn_in(window, async move |view, cx| {
            let result =
                cx.background_executor().spawn(async move { validate_directory(&directory) }).await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.finish_directory_selection();
                match result {
                    Ok(directory) => {
                        this.with_sidebar(|sidebar| sidebar.list_mut().expand_project(&directory));
                        if let Some(host) = this.tab_host().clone() {
                            window.defer(cx, move |window, cx| {
                                let _ = host.update(cx, |host, cx| {
                                    host.new_tab_in_directory(directory, window, cx);
                                });
                            });
                        } else {
                            this.start_new_chat(directory, window, cx);
                        }
                    },
                    Err(error) => {
                        crate::error::report("无法在项目目录新建会话", &error, window, cx);
                    },
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub fn start_new_chat(
        &mut self,
        ui: &mut SessionParts<'_>,
        directory: impl Into<PathBuf>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if !self.can_start_new_chat(ui) {
            return;
        }
        self.dismiss_permissions();
        self.save_draft(ui, cx);
        if self.disconnected {
            self.receiver = None;
            self.bridge = None;
        }
        self.reset_chat(ui);
        self.cwd = directory.into();
        ui.git_panel.set_directory(self.cwd.clone());
        ui.git_panel.watch_info(cx);
        self.session_directory_selected = true;
        self.disconnected = false;
        ui.composer.input().update(cx, |input, cx| {
            input.set_value("", window, cx);
            input.focus(window, cx);
        });
        self.connect(ui, window, cx);
        self.creating_session = true;
        if !self
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.send(Command::NewSession(self.cwd.clone())).is_ok())
        {
            self.apply_event(
                ui,
                Event::Disconnected("无法创建会话，请重新连接后重试。".into()),
                window,
                cx,
            );
        }
        cx.notify();
    }

    pub fn refresh_sessions(&mut self, ui: &mut SessionParts<'_>, cx: &mut Context<SessionView>) {
        if self.disconnected || !ui.sidebar.list_mut().is_supported() {
            return;
        }
        if ui.sidebar.list_mut().is_loading()
            || (ui.deletion.is_busy() || ui.management_pending)
            || self.loading_session.is_some()
        {
            ui.sidebar.list_mut().defer_refresh();
            return;
        }
        if !ui.sidebar.list_mut().begin_refresh() {
            return;
        }
        if !self.bridge.as_ref().is_some_and(|bridge| {
            bridge.send(Command::ListSessions(ui.sidebar.list().archive_filter())).is_ok()
        }) {
            ui.sidebar
                .list_mut()
                .fail_refresh("无法获取会话列表，请新建对话重新连接 Aries。".into());
        }
        cx.notify();
    }
}
