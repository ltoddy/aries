use std::fmt;
use std::path::{Path, PathBuf};

use agent_client_protocol::AcpAgentConfig;
use agent_client_protocol::v1::schema::{
    PermissionOptionId, SessionId, SessionInfo, SetSessionConfigOptionRequest,
};
use gpui_kit::component::button::ButtonVariant;
use gpui_kit::component::{ActiveTheme, WindowExt, h_resizable, resizable_panel};
use gpui_kit::prelude::*;
use gpui_kit::*;
use jiff::{SignedDuration, Timestamp};

use crate::acp::Event;
use crate::workspace::WorkspaceView;
use crate::workspace::conversation::prompt_queue::PromptQueue;
use crate::workspace::conversation::timeline::Timeline;
use crate::workspace::conversation::{CommandMenuAction, Composer};
use crate::workspace::files::OpenFile;
use crate::workspace::git::GitPanel;
use crate::workspace::session::configuration::Configuration;
use crate::workspace::session::execution::{Execution, SessionParts};
use crate::workspace::session::execution_context::ExecutionState;
use crate::workspace::session::model::SessionDeletion;
use crate::workspace::settings::SendKey;
use crate::workspace::settings::palette::Palette;
use crate::workspace::sidebar::Sidebar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionRenderMode {
    Full,
    Sidebar,
    Main,
}

pub mod management;
mod plan;
use self::management::SessionManagement;

pub struct SessionView {
    execution: Execution,
    management: SessionManagement,
    configuration: Configuration,
    deletion: SessionDeletion,
    composer: Composer,
    timeline: Timeline,
    plan_collapsed: bool,
    git_panel: GitPanel,
    sidebar: Sidebar,
    tab_host: Option<WeakEntity<WorkspaceView>>,
    render_mode: SessionRenderMode,
    tab_session_request: Option<SessionInfo>,
    tab_new_request: bool,
    _subscriptions: Vec<Subscription>,
}

impl fmt::Debug for SessionView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionView")
            .field("cwd", &self.execution.directory())
            .field("session_id", &self.execution.session_id())
            .field("message_count", &self.execution.conversation().messages().len())
            .field("disconnected", &self.execution.is_disconnected())
            .finish_non_exhaustive()
    }
}
impl SessionView {
    pub fn new(
        cwd: impl Into<PathBuf>,
        agent: AcpAgentConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let cwd = cwd.into();
        let composer = Composer::new(window, cx);
        let changes = cx.subscribe_in(composer.input(), window, |this, _, event, _, cx| {
            this.composer.input_event(event);
            cx.notify();
        });
        let quit = cx.on_app_quit(|this, _| {
            let bridge = this.execution.shutdown();
            async move {
                if let Some(bridge) = bridge {
                    bridge.shutdown().await;
                }
            }
        });
        let escape = cx.observe_keystrokes(|this, event, _, cx| {
            if event.keystroke.key == "escape" {
                this.cancel_session_deletion(cx);
            }
        });
        cx.defer_in(window, |this, window, cx| {
            this.git_panel.watch_info(cx);
            this.connect(window, cx);
        });
        Self {
            git_panel: GitPanel::new(cwd.clone(), cx),
            execution: Execution::new(cwd, agent),
            configuration: Configuration::default(),
            deletion: SessionDeletion::default(),
            management: SessionManagement::new(),
            composer,
            timeline: Timeline::new(cx),
            plan_collapsed: false,
            sidebar: Sidebar::new(),
            tab_host: None,
            render_mode: SessionRenderMode::Full,
            tab_session_request: None,
            tab_new_request: false,
            _subscriptions: vec![changes, quit, escape],
        }
    }
    fn coordinate<R>(
        &mut self,
        operation: impl FnOnce(&mut Execution, &mut SessionParts<'_>) -> R,
    ) -> R {
        let mut parts = SessionParts::new(
            &mut self.composer,
            &mut self.timeline,
            &mut self.sidebar,
            &mut self.git_panel,
            &mut self.configuration,
            &mut self.deletion,
        )
        .for_presentation(self.tab_host.clone(), self.render_mode)
        .with_management_pending(self.management.is_pending());
        operation(&mut self.execution, &mut parts)
    }
    pub fn execution(&self) -> &Execution {
        &self.execution
    }
    pub fn configuration(&self) -> &Configuration {
        &self.configuration
    }
    pub fn deletion(&self) -> &SessionDeletion {
        &self.deletion
    }
    pub fn git_panel(&self) -> &GitPanel {
        &self.git_panel
    }
    pub fn with_git_panel<R>(&mut self, operation: impl FnOnce(&mut GitPanel) -> R) -> R {
        operation(&mut self.git_panel)
    }
    pub fn with_sidebar<R>(&mut self, operation: impl FnOnce(&mut Sidebar) -> R) -> R {
        operation(&mut self.sidebar)
    }
    pub fn with_timeline<R>(
        &mut self,
        operation: impl FnOnce(
            &mut Timeline,
            &crate::workspace::conversation::model::Conversation,
            Option<&SessionId>,
        ) -> R,
    ) -> R {
        operation(&mut self.timeline, self.execution.conversation(), self.execution.session_id())
    }
    pub fn with_composer<R>(&mut self, operation: impl FnOnce(&mut Composer) -> R) -> R {
        operation(&mut self.composer)
    }
    pub fn attachments_available(&self) -> bool {
        let capabilities = self.execution.prompt_capabilities();
        self.execution.session_id().is_some()
            && !self.execution.is_disconnected()
            && self.execution.is_prompt_available(
                (self.deletion.is_busy() || self.management.is_pending())
                    || self.configuration.is_pending()
                    || self.composer.is_loading(),
            )
            && (capabilities.image || capabilities.audio || capabilities.embedded_context)
    }
    pub fn input(&self) -> &Entity<gpui_kit::component::input::TextareaState> {
        self.composer.input()
    }
    pub fn tab_host(&self) -> Option<WeakEntity<WorkspaceView>> {
        self.tab_host.clone()
    }
    pub fn open_file(&mut self, target: OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(host) = self.tab_host() {
            let owner = cx.weak_entity();
            window.defer(cx, move |window, cx| {
                let _ = host.update(cx, |host, cx| {
                    if host.is_active_session(&owner) {
                        host.open_file(target, window, cx);
                    }
                });
            });
        }
    }
    pub fn uses_sidebar_tab_host(&self) -> bool {
        self.render_mode == SessionRenderMode::Sidebar && self.tab_host.is_some()
    }
    pub fn is_session_idle(&self) -> bool {
        self.execution.is_idle(
            (self.deletion.is_busy() || self.management.is_pending())
                || self.configuration.is_pending()
                || self.composer.is_loading(),
        )
    }
    pub fn can_start_new_chat(&self) -> bool {
        self.is_session_idle()
    }
    pub fn is_prompt_ready(&self) -> bool {
        self.execution.is_prompt_available(
            (self.deletion.is_busy() || self.management.is_pending())
                || self.configuration.is_pending(),
        )
    }
    pub fn can_send(&self, cx: &App) -> bool {
        self.is_prompt_ready()
            && !self.execution.is_prompt_queue_full()
            && self.composer.has_content(cx)
    }
    pub fn can_load_sessions(&self) -> bool {
        self.execution.supports_load()
            && !self.execution.is_disconnected()
            && self.is_session_idle()
            && !self.sidebar.list().is_loading()
    }
    pub fn can_manage_sessions(&self) -> bool {
        !self.execution.is_disconnected()
            && self.is_session_idle()
            && !self.sidebar.list().is_loading()
            && self.sidebar.list().is_supported()
    }
    pub fn commands_available(&self) -> bool {
        !self.execution.is_disconnected()
            && !self.execution.is_loading()
            && !self.execution.is_creating()
            && !self.execution.is_choosing_directory()
    }
    pub fn send_key(&self, cx: &App) -> SendKey {
        self.tab_host
            .as_ref()
            .and_then(|host| host.upgrade())
            .map(|host| host.read(cx).preferences(cx).send_key())
            .unwrap_or_default()
    }
}

mod commands;
mod render;
mod sessions;

impl SessionView {
    pub fn set_tab_host(&mut self, host: WeakEntity<WorkspaceView>) {
        self.tab_host = Some(host);
    }
    pub fn set_render_mode(&mut self, mode: SessionRenderMode) {
        self.render_mode = mode;
    }
    pub fn request_new_tab(&mut self, cx: &mut Context<Self>) {
        self.tab_new_request = true;
        cx.notify();
    }
    pub fn take_new_tab_request(&mut self) -> bool {
        std::mem::take(&mut self.tab_new_request)
    }
    pub fn request_session_tab(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        self.tab_session_request = Some(session);
        cx.notify();
    }
    pub fn take_session_tab_request(&mut self) -> Option<SessionInfo> {
        self.tab_session_request.take()
    }
    pub fn take_created_sidebar_session(&mut self) -> Option<SessionInfo> {
        self.sidebar.take_created_session()
    }
    pub fn receive_created_sidebar_session(
        &mut self,
        session: SessionInfo,
        cx: &mut Context<Self>,
    ) {
        self.sidebar.list_mut().expand_project(&session.cwd);
        if !self.sidebar.list().show_archived() {
            self.sidebar.motion_mut().add_created(session);
        }
        self.refresh_sessions(cx);
        cx.notify();
    }
    pub fn matches_session(&self, id: &SessionId) -> bool {
        self.execution.matches_session(id)
    }
    pub fn active_session_id(&self) -> Option<&SessionId> {
        self.execution.active_session_id()
    }
    pub fn tab_title(&self) -> String {
        let directory = self.execution.tab_directory();
        let project = directory.file_name().unwrap_or(directory.as_os_str()).to_string_lossy();
        crate::workspace::conversation::single_line(format!(
            "{project} · {}",
            self.execution.conversation().title()
        ))
    }
    pub fn directory(&self) -> &Path {
        self.execution.directory()
    }
}
