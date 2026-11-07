use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use agent_client_protocol::v1::schema::{
    LoadSessionRequest, LoadSessionResponse, PermissionOptionId, SessionId, SessionInfo,
    SessionNotification, SessionUpdate, StopReason,
};
use agent_client_protocol::{AcpAgentConfig, Error};
use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::*;

use crate::acp::{Bridge, Command, DeletionReport, Event, Permission};
use crate::workspace::conversation::Composer;
use crate::workspace::conversation::model::Conversation;
use crate::workspace::conversation::timeline::Timeline;
use crate::workspace::git::GitPanel;
use crate::workspace::session::configuration::Configuration;
use crate::workspace::session::model::{SessionDeletion, SessionLoad};
use crate::workspace::sidebar::Sidebar;
use crate::workspace::{SessionRenderMode, SessionView, WorkspaceView};

pub struct Execution {
    cwd: PathBuf,
    agent: AcpAgentConfig,
    conversation: Conversation,
    bridge: Option<Bridge>,
    receiver: Option<Task<()>>,
    session_id: Option<SessionId>,
    queued_session_load: Option<SessionInfo>,
    pending_updates: Vec<SessionNotification>,
    drafts: HashMap<SessionId, String>,
    loading_session: Option<SessionLoad>,
    creating_session: bool,
    load_supported: bool,
    disconnected: bool,
    stopping: bool,
    status: String,
    tokens: Option<u64>,
    choosing_directory: bool,
    session_directory_selected: bool,
    permissions: VecDeque<Permission>,
}

pub struct SessionParts<'a> {
    composer: &'a mut Composer,
    timeline: &'a mut Timeline,
    sidebar: &'a mut Sidebar,
    git_panel: &'a mut GitPanel,
    configuration: &'a mut Configuration,
    deletion: &'a mut SessionDeletion,
    tab_host: Option<WeakEntity<WorkspaceView>>,
    render_mode: SessionRenderMode,
}
impl<'a> SessionParts<'a> {
    pub fn new(
        composer: &'a mut Composer,
        timeline: &'a mut Timeline,
        sidebar: &'a mut Sidebar,
        git_panel: &'a mut GitPanel,
        configuration: &'a mut Configuration,
        deletion: &'a mut SessionDeletion,
    ) -> Self {
        Self {
            composer,
            timeline,
            sidebar,
            git_panel,
            configuration,
            deletion,
            tab_host: None,
            render_mode: SessionRenderMode::Full,
        }
    }

    pub fn for_presentation(
        mut self,
        tab_host: Option<WeakEntity<WorkspaceView>>,
        render_mode: SessionRenderMode,
    ) -> Self {
        self.tab_host = tab_host;
        self.render_mode = render_mode;
        self
    }
}

impl Execution {
    fn finish_deletion(
        &mut self,
        ui: &mut SessionParts<'_>,
        result: DeletionReport,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        let (deleted, failed) = result.into_parts();
        let deleted: HashSet<_> = deleted.into_iter().collect();
        ui.sidebar.begin_sidebar_exit(
            &deleted,
            self.session_id.as_ref(),
            &self.cwd,
            self.conversation.title(),
            cx,
        );
        self.drafts.retain(|id, _| !deleted.contains(id));
        if self.session_id.as_ref().is_some_and(|id| deleted.contains(id)) {
            self.reset_chat(ui);
            ui.composer.input().update(cx, |input, cx| input.set_value("", window, cx));
        }
        ui.sidebar.list_mut().remove_sessions(&deleted);
        let notice = if failed.is_empty() {
            format!("已删除 {} 个会话", deleted.len())
        } else {
            format!("已删除 {} 个，{} 个删除失败", deleted.len(), failed.len())
        };
        let error = (!failed.is_empty()).then(|| {
            failed
                .iter()
                .map(|(id, error)| {
                    let title = ui
                        .sidebar
                        .list()
                        .session(id)
                        .and_then(|session| session.title.as_deref())
                        .filter(|title| !title.trim().is_empty())
                        .map(str::to_owned)
                        .unwrap_or_else(|| id.to_string());
                    format!("{title}：{error}")
                })
                .collect::<Vec<_>>()
                .join("\n")
        });
        let notification = if failed.is_empty() {
            Notification::success(notice)
        } else {
            Notification::warning(notice)
        };
        window.push_notification(notification.autohide(true), cx);
        ui.deletion.finish_deletion(error);
        if !deleted.is_empty()
            && let Some(host) = ui.tab_host.clone()
        {
            cx.defer(move |cx| {
                let _ = host.update(cx, |host, cx| host.close_deleted_sessions(&deleted, cx));
            });
        }
        self.refresh_sessions(ui, cx);
        cx.notify();
    }
    pub fn new(cwd: PathBuf, agent: AcpAgentConfig) -> Self {
        Self {
            cwd,
            agent,
            conversation: Conversation::default(),
            bridge: None,
            receiver: None,
            session_id: None,
            queued_session_load: None,
            pending_updates: Vec::new(),
            drafts: HashMap::new(),
            loading_session: None,
            creating_session: false,
            load_supported: false,
            disconnected: false,
            stopping: false,
            status: "就绪".into(),
            tokens: None,
            choosing_directory: false,
            session_directory_selected: false,
            permissions: VecDeque::new(),
        }
    }
    pub fn status(&self) -> &str {
        &self.status
    }
    pub fn conversation(&self) -> &Conversation {
        &self.conversation
    }
    pub fn directory(&self) -> &Path {
        &self.cwd
    }
    pub fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }
    pub fn is_disconnected(&self) -> bool {
        self.disconnected
    }
    pub fn is_stopping(&self) -> bool {
        self.stopping
    }
    pub fn is_loading(&self) -> bool {
        self.loading_session.is_some()
    }
    pub fn is_creating(&self) -> bool {
        self.creating_session
    }
    pub fn is_choosing_directory(&self) -> bool {
        self.choosing_directory
    }
    pub fn directory_selected(&self) -> bool {
        self.session_id.is_some() || self.session_directory_selected
    }
    pub fn tokens(&self) -> Option<u64> {
        self.tokens
    }
    pub fn permission(&self) -> Option<&Permission> {
        self.permissions.front()
    }
    pub fn bridge(&self) -> Option<&Bridge> {
        self.bridge.as_ref()
    }
    pub fn set_config_options(
        &mut self,
        options: Vec<agent_client_protocol::v1::schema::SessionConfigOption>,
    ) {
        self.conversation.set_config_options(options);
    }
    pub fn finish_directory_selection(&mut self) {
        self.choosing_directory = false;
    }
    pub fn shutdown(&mut self) -> Option<Bridge> {
        self.dismiss_permissions();
        self.bridge.take()
    }
    pub fn loading_id(&self) -> Option<&SessionId> {
        self.loading_session
            .as_ref()
            .map(|l| l.session_id())
            .or_else(|| self.queued_session_load.as_ref().map(|s| &s.session_id))
    }
    pub fn active_session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref().or_else(|| self.loading_id())
    }
    pub fn matches_session(&self, id: &SessionId) -> bool {
        self.session_id.as_ref() == Some(id)
            || self.loading_session.as_ref().is_some_and(|l| l.session_id() == id)
            || self.queued_session_load.as_ref().is_some_and(|s| &s.session_id == id)
    }
    pub fn queue_session_load(
        &mut self,
        ui: &mut SessionParts<'_>,
        session: SessionInfo,
        cx: &mut Context<SessionView>,
    ) {
        self.conversation.set_title(session.title.clone());
        if self.load_supported && !self.disconnected && self.is_session_idle(ui) {
            self.load_session(ui, session, cx);
        } else {
            self.queued_session_load = Some(session);
            cx.notify();
        }
    }
    pub fn sync_active(
        &mut self,
        id: Option<SessionId>,
        cwd: Option<PathBuf>,
        title: Option<String>,
    ) -> bool {
        let changed = self.session_id != id
            || cwd.as_ref().is_some_and(|p| &self.cwd != p)
            || title.as_ref().is_some_and(|t| self.conversation.title() != *t);
        self.session_id = id;
        if let Some(cwd) = cwd {
            self.cwd = cwd;
        }
        if let Some(title) = title {
            self.conversation.set_title(Some(title));
        }
        changed
    }

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
        let Some(prompt) = self.conversation.begin_turn(&ui.composer.input().read(cx).value())
        else {
            return;
        };
        if self.bridge.is_none() {
            self.connect(ui, window, cx);
        }
        if !ui.sidebar.list_mut().is_connecting() && !self.disconnected {
            self.status = "思考中…".into();
        }
        if self
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.send(Command::Prompt { session_id, text: prompt }).is_ok())
        {
            ui.composer.input().update(cx, |input, cx| input.set_value("", window, cx));
        } else {
            self.apply_event(
                ui,
                Event::Disconnected("无法发送消息，请新建对话".into()),
                window,
                cx,
            );
        }
        ui.timeline.list().set_follow_mode(FollowMode::Tail);
        cx.notify();
    }

    pub fn is_idle(&self, blocked: bool) -> bool {
        !blocked
            && !self.conversation.is_busy()
            && self.loading_session.is_none()
            && !self.creating_session
            && !self.choosing_directory
    }
    pub fn is_session_idle(&self, ui: &SessionParts<'_>) -> bool {
        self.is_idle(ui.deletion.is_busy() || ui.configuration.is_pending())
    }
    pub fn can_send(&self, ui: &SessionParts<'_>, cx: &App) -> bool {
        self.session_id.is_some()
            && self.is_session_idle(ui)
            && !self.disconnected
            && !ui.composer.input().read(cx).value().trim().is_empty()
    }
    pub fn supports_load(&self) -> bool {
        self.load_supported
    }
    pub fn save_draft(&mut self, ui: &mut SessionParts<'_>, cx: &App) {
        if let Some(id) = &self.session_id {
            let draft = ui.composer.input().read(cx).value().to_string();
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
        if self.bridge.as_ref().is_some_and(|bridge| bridge.send(Command::Cancel).is_ok()) {
            self.stopping = true;
            self.status = "正在停止…".into();
        } else {
            self.apply_event(ui, Event::Disconnected("连接已关闭，请新建对话".into()), window, cx);
        }
        cx.notify();
    }

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
        ui.timeline.reset();
        self.session_id = None;
        self.queued_session_load = None;
        self.pending_updates.clear();
        self.session_directory_selected = false;
        self.loading_session = None;
        self.creating_session = false;
        self.stopping = false;
        self.status = "就绪".into();
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
        ui.sidebar.list_mut().connect();
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
            }
            Err(error) => {
                self.apply_event(
                    ui,
                    Event::Disconnected(format!("无法启动 Agent：{error}")),
                    window,
                    cx,
                );
            }
        }
        cx.notify();
    }

    pub fn apply_event(
        &mut self,
        ui: &mut SessionParts<'_>,
        event: Event,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        match event {
            Event::Connected { list_sessions, load_session } => {
                ui.sidebar.list_mut().connected(list_sessions);
                self.load_supported = load_session;
                if !self.stopping && !self.creating_session {
                    self.status =
                        if self.conversation.is_busy() { "思考中…" } else { "就绪" }.into();
                }
                if let Some(session) = self.queued_session_load.take() {
                    self.load_session(ui, session, cx);
                }
                if ui.render_mode != crate::workspace::SessionRenderMode::Main {
                    self.refresh_sessions(ui, cx);
                }
            }
            Event::Sessions(result) => {
                let received = ui.sidebar.list_mut().is_loading() && result.is_ok();
                ui.sidebar.list_mut().receive(result);
                if received {
                    let entries = ui.sidebar.list().entries().to_vec();
                    ui.sidebar.motion_mut().receive(entries, cx.background_executor().now());
                    ui.sidebar.sync_sidebar_rows(
                        self.session_id.as_ref(),
                        &self.cwd,
                        self.conversation.title(),
                        cx,
                    );
                }
                if ui.sidebar.list_mut().take_refresh_request() {
                    self.refresh_sessions(ui, cx);
                }
            }
            Event::SessionsDeleted(result) => self.finish_deletion(ui, result, window, cx),
            Event::SessionLoaded { session_id, result } => {
                self.finish_session_load(ui, session_id, result, window, cx);
            }
            Event::SessionConfigSet { session_id, config_id, result } => {
                ui.configuration.finish_session_config(self, session_id, config_id, result, cx);
            }
            Event::Ready(response) => {
                let creating_session = std::mem::take(&mut self.creating_session);
                self.session_id = Some(response.session_id.clone());
                if creating_session {
                    let session = agent_client_protocol::v1::schema::SessionInfo::new(
                        response.session_id,
                        self.cwd.clone(),
                    )
                    .title(self.conversation.title());
                    ui.sidebar
                        .motion_mut()
                        .add_created(session.clone(), cx.background_executor().now());
                    ui.sidebar.set_created_session(session);
                }
                self.conversation.end_segment();
                self.status = if self.conversation.is_busy() { "思考中…" } else { "就绪" }.into();
                for notification in std::mem::take(&mut self.pending_updates) {
                    self.receive_update(ui, notification);
                }
                if let Some(options) = response.config_options {
                    self.conversation.set_config_options(options);
                }
                if creating_session && ui.render_mode != crate::workspace::SessionRenderMode::Main {
                    self.refresh_sessions(ui, cx);
                }
            }
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
                self.conversation.finish(result.err().map(|error| error.to_string()));
                if ui.render_mode != crate::workspace::SessionRenderMode::Main {
                    self.refresh_sessions(ui, cx);
                }
            }
            Event::Disconnected(error) => {
                self.dismiss_permissions();
                self.stopping = false;
                self.disconnected = true;
                self.load_supported = false;
                self.loading_session = None;
                self.creating_session = false;
                self.pending_updates.clear();
                ui.configuration.disconnect();
                ui.sidebar.list_mut().disconnect(error.clone());
                ui.deletion.disconnect();
                self.status = "连接已断开".into();
                self.conversation.finish(Some(format!("{error}\n请新建对话重新连接 Aries。")));
            }
        }
        if ui.render_mode != crate::workspace::SessionRenderMode::Main {
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
        if self.conversation.is_busy() && !self.stopping && self.permissions.is_empty() {
            match &notification.update {
                SessionUpdate::AgentThoughtChunk(_) => self.status = "思考中…".into(),
                SessionUpdate::AgentMessageChunk(_) => self.status = "正在回复…".into(),
                SessionUpdate::ToolCall(tool) => self.status = tool.title.clone(),
                _ => {}
            }
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

    pub fn choose_permission(&mut self, option: PermissionOptionId, cx: &mut Context<SessionView>) {
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

    pub fn can_load_sessions(&self, ui: &mut SessionParts<'_>) -> bool {
        self.load_supported
            && !self.disconnected
            && self.is_session_idle(ui)
            && !ui.sidebar.list_mut().is_loading()
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
            self.status = "加载失败".into();
            self.conversation.finish(Some("无法发送加载请求，请重新连接后重试。".into()));
            cx.notify();
            return;
        }
        self.dismiss_permissions();
        self.conversation.finish(None);
        self.loading_session = Some(SessionLoad::new(session));
        self.status = "正在加载会话…".into();
        cx.notify();
    }

    pub fn finish_session_load(
        &mut self,
        ui: &mut SessionParts<'_>,
        id: SessionId,
        result: Result<LoadSessionResponse, Error>,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if !self.loading_session.as_ref().is_some_and(|loading| loading.session_id() == &id) {
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
                ui.composer.input().update(cx, |input, cx| input.set_value(draft, window, cx));
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
            }
            Err(error) => {
                self.status = "加载失败".into();
                self.conversation.finish(Some(format!("无法加载会话：{error}")));
            }
        }
        if ui.sidebar.list_mut().take_refresh_request() {
            self.refresh_sessions(ui, cx);
        }
        cx.notify();
    }

    pub fn can_start_new_chat(&self, ui: &mut SessionParts<'_>) -> bool {
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
            let result: anyhow::Result<Option<PathBuf>> = async {
                let Some(path) = selection.await??.and_then(|paths| paths.into_iter().next())
                else {
                    return Ok(None);
                };
                let directory = cx
                    .background_executor()
                    .spawn(async move { validate_directory(&path) })
                    .await?;
                Ok(Some(directory))
            }
            .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.finish_directory_selection();
                match result {
                    Ok(Some(directory)) => {
                        if let Some(host) = &this.tab_host() {
                            let _ = host.update(cx, |host, cx| {
                                host.new_tab_in_directory(directory, window, cx);
                            });
                        } else {
                            this.start_new_chat(directory, window, cx);
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        window.push_notification(
                            Notification::error(format!("无法选择项目目录：{error}")),
                            cx,
                        );
                    }
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
        directory: PathBuf,
        window: &mut Window,
        cx: &mut Context<SessionView>,
    ) {
        if !self.can_start_new_chat(ui) {
            return;
        }
        self.choosing_directory = true;
        cx.spawn_in(window, async move |view, cx| {
            let (result, directory) = cx
                .background_executor()
                .spawn(async move { (validate_directory(&directory), directory) })
                .await;
            let _ = view.update_in(cx, |this, window, cx| {
                this.finish_directory_selection();
                match result {
                    Ok(directory) => {
                        this.with_sidebar(|sidebar| sidebar.list_mut().expand_project(&directory));
                        if let Some(host) = &this.tab_host() {
                            let _ = host.update(cx, |host, cx| {
                                host.new_tab_in_directory(directory, window, cx);
                            });
                        } else {
                            this.start_new_chat(directory, window, cx);
                        }
                    }
                    Err(error) => window.push_notification(
                        Notification::error(format!(
                            "无法在目录 {} 新建会话：{error}。请检查目录是否存在且可访问。",
                            directory.display()
                        )),
                        cx,
                    ),
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
        directory: PathBuf,
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
        self.cwd = directory;
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
        self.status = "正在创建会话…".into();
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
            || ui.deletion.is_busy()
            || self.loading_session.is_some()
        {
            ui.sidebar.list_mut().defer_refresh();
            return;
        }
        if !ui.sidebar.list_mut().begin_refresh() {
            return;
        }
        if !self.bridge.as_ref().is_some_and(|bridge| bridge.send(Command::ListSessions).is_ok()) {
            ui.sidebar
                .list_mut()
                .fail_refresh("无法获取会话列表，请新建对话重新连接 Aries。".into());
        }
        cx.notify();
    }
}
fn validate_directory(path: &Path) -> std::io::Result<PathBuf> {
    let directory = path.canonicalize()?;
    if !directory.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::NotADirectory, "请选择文件夹"));
    }
    Ok(directory)
}
