use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use agent_client_protocol::v1::schema::{
    ContentBlock, LoadSessionRequest, LoadSessionResponse, PermissionOptionId, PromptCapabilities,
    SessionId, SessionInfo, SessionNotification, SessionUpdate, StopReason,
};
use agent_client_protocol::{AcpAgentConfig, Error};
use gpui_kit::component::WindowExt;
use gpui_kit::component::notification::Notification;
use gpui_kit::*;
use thiserror::Error as ThisError;

use crate::acp::{Bridge, Command, DeletionReport, Event, Permission};
use crate::workspace::conversation::Composer;
use crate::workspace::conversation::draft::Draft;
use crate::workspace::conversation::model::Conversation;
use crate::workspace::conversation::prompt_queue::PromptQueue;
use crate::workspace::conversation::timeline::Timeline;
use crate::workspace::git::GitPanel;
use crate::workspace::session::configuration::Configuration;
use crate::workspace::session::model::{SessionDeletion, SessionLoad};
use crate::workspace::sidebar::Sidebar;
use crate::workspace::{SessionRenderMode, SessionView, WorkspaceView};

mod event;
mod lifecycle;
mod prompt;

pub struct Execution {
    cwd: PathBuf,
    agent: AcpAgentConfig,
    conversation: Conversation,
    bridge: Option<Bridge>,
    receiver: Option<Task<()>>,
    session_id: Option<SessionId>,
    queued_session_load: Option<SessionInfo>,
    pending_updates: Vec<SessionNotification>,
    drafts: HashMap<SessionId, Draft>,
    prompt_queues: HashMap<SessionId, PromptQueue>,
    prompt_capabilities: PromptCapabilities,
    loading_session: Option<SessionLoad>,
    creating_session: bool,
    load_supported: bool,
    disconnected: bool,
    stopping: bool,
    pet_completion: u64,
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
    management_pending: bool,
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
            management_pending: false,
        }
    }

    pub fn with_management_pending(mut self, pending: bool) -> Self {
        self.management_pending = pending;
        self
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
        ui.sidebar.remove_sidebar_sessions(
            &deleted,
            self.session_id.as_ref(),
            &self.cwd,
            self.conversation.title(),
            cx,
        );
        self.drafts.retain(|id, _| !deleted.contains(id));
        self.prompt_queues.retain(|id, _| !deleted.contains(id));
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
                    crate::error::log(&format!("无法删除会话 {title}"), error);
                    format!("{title}：{}", crate::error::details(error))
                })
                .collect::<Vec<_>>()
                .join("\n")
        });
        if let Some((_, error)) = failed.first() {
            crate::error::report(notice, error, window, cx);
        } else {
            window.push_notification(Notification::success(notice).autohide(true), cx);
        }
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
    pub fn new(cwd: impl Into<PathBuf>, agent: AcpAgentConfig) -> Self {
        Self {
            cwd: cwd.into(),
            agent,
            conversation: Conversation::default(),
            bridge: None,
            receiver: None,
            session_id: None,
            queued_session_load: None,
            pending_updates: Vec::new(),
            drafts: HashMap::new(),
            prompt_queues: HashMap::new(),
            prompt_capabilities: PromptCapabilities::new(),
            loading_session: None,
            creating_session: false,
            load_supported: false,
            disconnected: false,
            stopping: false,
            pet_completion: 0,
            tokens: None,
            choosing_directory: false,
            session_directory_selected: false,
            permissions: VecDeque::new(),
        }
    }
    pub fn prompt_capabilities(&self) -> &PromptCapabilities {
        &self.prompt_capabilities
    }
    pub fn set_session_title(&mut self, id: &SessionId, title: String) {
        if self.matches_session(id) {
            self.conversation.set_title(Some(title.clone()));
        }
        if let Some(session) = &mut self.loading_session
            && session.session_id() == id
        {
            session.set_title(title.clone());
        }
        if let Some(session) = &mut self.queued_session_load
            && &session.session_id == id
        {
            session.title = Some(title);
        }
    }
    pub fn conversation(&self) -> &Conversation {
        &self.conversation
    }
    pub fn directory(&self) -> &Path {
        &self.cwd
    }
    pub fn tab_directory(&self) -> &Path {
        self.loading_session
            .as_ref()
            .map(|session| session.directory())
            .or_else(|| self.queued_session_load.as_ref().map(|session| session.cwd.as_path()))
            .unwrap_or(&self.cwd)
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
    pub fn pet_completion(&self) -> u64 {
        self.pet_completion
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

    pub fn choose_permission(&mut self, option: PermissionOptionId, cx: &mut Context<SessionView>) {
        if let Some(permission) = self.permissions.pop_front() {
            permission.respond(Some(option));
        }
        cx.notify();
    }

    pub fn dismiss_permissions(&mut self) {
        for permission in self.permissions.drain(..) {
            permission.respond(None);
        }
    }
}

#[derive(Debug, ThisError)]
enum SelectDirectoryError {
    #[error("等待目录选择结果失败")]
    Wait { source: Box<dyn std::error::Error + Send + Sync> },
    #[error("打开目录选择窗口失败")]
    Open { source: Box<dyn std::error::Error + Send + Sync> },
    #[error("所选项目目录不可用")]
    Validate { source: ValidateDirectoryError },
}

impl SelectDirectoryError {
    fn wait(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Wait { source }
    }

    fn open(source: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Open { source }
    }

    fn validate(source: ValidateDirectoryError) -> Self {
        Self::Validate { source }
    }
}

#[derive(Debug, ThisError)]
enum ValidateDirectoryError {
    #[error("无法解析目录 {}", path.display())]
    Canonicalize { path: PathBuf, source: std::io::Error },
    #[error("无法访问目录 {}", path.display())]
    Metadata { path: PathBuf, source: std::io::Error },
    #[error("请选择文件夹：{}", path.display())]
    NotDirectory { path: PathBuf },
}

impl ValidateDirectoryError {
    fn canonicalize(path: &Path, source: std::io::Error) -> Self {
        Self::Canonicalize { path: path.to_path_buf(), source }
    }

    fn metadata(path: &Path, source: std::io::Error) -> Self {
        Self::Metadata { path: path.to_path_buf(), source }
    }

    fn not_directory(path: PathBuf) -> Self {
        Self::NotDirectory { path }
    }
}

fn validate_directory(path: impl AsRef<Path>) -> Result<PathBuf, ValidateDirectoryError> {
    let path = path.as_ref();
    let directory =
        path.canonicalize().map_err(|source| ValidateDirectoryError::canonicalize(path, source))?;
    let metadata = directory
        .metadata()
        .map_err(|source| ValidateDirectoryError::metadata(&directory, source))?;
    if !metadata.is_dir() {
        return Err(ValidateDirectoryError::not_directory(directory));
    }
    Ok(directory)
}
