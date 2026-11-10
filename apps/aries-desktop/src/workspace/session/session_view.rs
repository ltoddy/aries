use std::fmt;
use std::path::{Path, PathBuf};

use agent_client_protocol::AcpAgentConfig;
use agent_client_protocol::v1::schema::{
    PermissionOptionId, SessionId, SessionInfo, SetSessionConfigOptionRequest,
};
use gpui_kit::component::{ActiveTheme, h_resizable, resizable_panel};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::acp::Event;
use crate::workspace::WorkspaceView;
use crate::workspace::conversation::timeline::Timeline;
use crate::workspace::conversation::{CommandMenuAction, Composer};
use crate::workspace::git::GitPanel;
use crate::workspace::session::configuration::Configuration;
use crate::workspace::session::execution::{Execution, SessionParts};
use crate::workspace::session::execution_context::ExecutionState;
use crate::workspace::session::model::SessionDeletion;
use crate::workspace::settings::palette::Palette;
use crate::workspace::settings::{Preferences, SendKey};
use crate::workspace::sidebar::Sidebar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionRenderMode {
    Full,
    Sidebar,
    Main,
}

pub struct SessionView {
    execution: Execution,
    configuration: Configuration,
    deletion: SessionDeletion,
    composer: Composer,
    timeline: Timeline,
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
        cwd: PathBuf,
        agent: AcpAgentConfig,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
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
            composer,
            timeline: Timeline::new(cx),
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
        .for_presentation(self.tab_host.clone(), self.render_mode);
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
            && self.is_session_idle()
            && (capabilities.image || capabilities.audio || capabilities.embedded_context)
    }
    pub fn input(&self) -> &Entity<gpui_kit::component::input::TextareaState> {
        self.composer.input()
    }
    pub fn tab_host(&self) -> Option<WeakEntity<WorkspaceView>> {
        self.tab_host.clone()
    }
    pub fn uses_sidebar_tab_host(&self) -> bool {
        self.render_mode == SessionRenderMode::Sidebar && self.tab_host.is_some()
    }
    pub fn is_session_idle(&self) -> bool {
        self.execution.is_idle(
            self.deletion.is_busy()
                || self.configuration.is_pending()
                || self.composer.is_loading(),
        )
    }
    pub fn can_start_new_chat(&self) -> bool {
        self.is_session_idle()
    }
    pub fn can_send(&self, cx: &App) -> bool {
        self.execution.session_id().is_some()
            && self.is_session_idle()
            && !self.execution.is_disconnected()
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
        self.preferences(cx).send_key()
    }
    pub fn preferences(&self, cx: &App) -> Preferences {
        self.tab_host
            .as_ref()
            .and_then(|host| host.upgrade())
            .map(|host| host.read(cx).preferences(cx).clone())
            .unwrap_or_default()
    }
    pub fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.send(parts, window, cx));
    }
    pub fn stop(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.stop(parts, window, cx));
    }
    pub fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.connect(parts, window, cx));
    }
    pub fn apply_event(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.apply_event(parts, event, window, cx));
    }
    pub fn new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.coordinate(|execution, parts| execution.new_chat(parts, window, cx));
    }
    pub fn new_chat_in_directory(
        &mut self,
        directory: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.coordinate(|execution, parts| {
            execution.new_chat_in_directory(parts, directory, window, cx)
        });
    }
    pub fn new_chat_in_current_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.new_chat_in_directory(self.execution.directory().to_path_buf(), window, cx);
    }
    pub fn start_new_chat(
        &mut self,
        directory: PathBuf,
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
        self.sidebar.motion_mut().add_created(session, cx.background_executor().now());
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
        crate::workspace::conversation::single_line(&self.execution.conversation().title())
    }
    pub fn directory(&self) -> &Path {
        self.execution.directory()
    }
    pub fn is_empty_tab(&self, cx: &App) -> bool {
        self.execution.session_id().is_none()
            && self.execution.loading_id().is_none()
            && self.execution.conversation().messages().is_empty()
            && self.composer.input().read(cx).value().trim().is_empty()
            && self.is_session_idle()
    }
    pub fn sidebar_snapshot(&self) -> (Option<SessionId>, PathBuf, String, Option<SessionId>) {
        (
            self.active_session_id().cloned(),
            self.directory().to_path_buf(),
            self.execution.conversation().title(),
            self.execution.loading_id().cloned(),
        )
    }
    pub fn sync_sidebar_active(
        &mut self,
        snapshot: Option<(Option<SessionId>, PathBuf, String, Option<SessionId>)>,
        cx: &mut Context<Self>,
    ) {
        let (id, cwd, title, loading) = match snapshot {
            Some((id, cwd, title, loading)) => (id, Some(cwd), Some(title), loading),
            None => (None, None, None, None),
        };
        let changed_loading = self.sidebar.loading_session() != loading.as_ref();
        self.sidebar.set_loading_session(loading);
        if self.execution.sync_active(id, cwd, title) || changed_loading {
            self.sync_sidebar_rows(cx);
            cx.notify();
        }
    }
    pub fn set_sidebar_session(&mut self, id: SessionId, cx: &mut Context<Self>) {
        if self.execution.session_id() != Some(&id) {
            self.execution.sync_active(Some(id), None, None);
            self.sync_sidebar_rows(cx);
            cx.notify();
        }
    }
    pub fn sync_sidebar_rows(&mut self, cx: &mut Context<Self>) {
        self.sidebar.sync_sidebar_rows(
            self.execution.session_id(),
            self.execution.directory(),
            self.execution.conversation().title(),
            cx,
        );
    }
    pub fn locate_current_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cwd = self.directory().to_path_buf();
        self.sidebar.locate_current_session(self.execution.session_id(), &cwd, window, cx);
    }
    pub fn session_ids(&self, cwd: Option<&Path>) -> Vec<SessionId> {
        let mut ids = self.sidebar.motion().session_ids(cwd);
        if cwd.is_none_or(|cwd| cwd == self.directory())
            && let Some(id) = self.execution.session_id()
            && !ids.contains(id)
        {
            ids.push(id.clone());
        }
        ids.retain(|id| !self.sidebar.motion().is_deleted(id));
        ids
    }
    pub fn prepare_session_deletion(
        &mut self,
        ids: Vec<SessionId>,
        anchor: String,
        cx: &mut Context<Self>,
    ) {
        if !self.can_manage_sessions() {
            return;
        }
        if self.deletion.prepare_available(ids, anchor, self.session_ids(None)) {
            cx.notify();
        }
    }

    pub fn confirm_session_deletion(&mut self, cx: &mut Context<Self>) {
        let blocked = self.execution.is_disconnected()
            || self.execution.conversation().is_busy()
            || self.sidebar.list().is_loading();
        if self.deletion.confirm(self.execution.bridge(), blocked) {
            cx.notify();
        }
    }

    pub fn cancel_session_deletion(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.deletion.cancel_pending() {
            return false;
        }
        if self.sidebar.list_mut().take_refresh_request() {
            self.refresh_sessions(cx);
        }
        cx.notify();
        true
    }
    pub fn render_session_actions(&self, p: Palette) -> impl IntoElement {
        crate::workspace::session::deletion::render_session_actions(self, p)
    }
    pub fn render_session_delete_popup(
        &self,
        anchor: impl AsRef<str>,
        p: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        crate::workspace::session::deletion::render_session_delete_popup(self, anchor, p, cx)
    }
    pub fn render_session_config(&self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        crate::workspace::session::config::render_session_config(self, p, cx)
    }
    pub fn render_execution_context(&self, p: Palette, cx: &Context<Self>) -> impl IntoElement {
        crate::workspace::session::execution_context::render_execution_context(self, p, cx)
    }
    pub fn execution_state(&self) -> ExecutionState {
        [
            (self.execution.is_disconnected(), ExecutionState::Disconnected),
            (self.execution.is_stopping(), ExecutionState::Stopping),
            (self.deletion.is_busy(), ExecutionState::Deleting),
            (self.execution.permission().is_some(), ExecutionState::Permission),
            (self.sidebar.list().is_connecting(), ExecutionState::Connecting),
            (self.execution.is_choosing_directory(), ExecutionState::ChoosingProject),
            (self.execution.loading_id().is_some(), ExecutionState::Loading),
            (self.execution.is_creating(), ExecutionState::Creating),
            (self.configuration.is_pending(), ExecutionState::Configuring),
            (self.execution.conversation().is_busy(), ExecutionState::Running),
            (self.execution.conversation().error().is_some(), ExecutionState::Failed),
        ]
        .into_iter()
        .find_map(|(active, state)| active.then_some(state))
        .unwrap_or(ExecutionState::Idle)
    }
    fn render_sidebar(
        &self,
        p: Palette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        self.sidebar.render(self, self.tab_host.clone(), p, window, cx)
    }
    fn render_main_area(&mut self, p: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let timeline = self.timeline.render_timeline(
            self.execution.conversation(),
            self.execution.is_loading(),
            p,
            cx,
        );
        let composer =
            self.composer.render(self, self.send_key(cx), self.commands_available(), p, cx);
        div().flex_1().min_w_0().h_full().flex().flex_col().child(timeline).child(composer)
    }
    fn render_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::new(cx.theme().mode);
        div()
            .size_full()
            .flex()
            .bg(p.background())
            .text_color(p.foreground())
            .font_family(cx.theme().font_family.clone())
            .text_sm()
            .child(
                h_resizable("layout")
                    .child(
                        resizable_panel()
                            .size(cx.theme().font_size * 18.)
                            .size_range(cx.theme().font_size * 12.5..cx.theme().font_size * 32.5)
                            .child(self.render_sidebar(p, window, cx)),
                    )
                    .child(resizable_panel().child(self.render_main_area(p, cx))),
            )
    }
}
impl Render for SessionView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sidebar.cleanup(cx);
        let p = Palette::new(cx.theme().mode);
        match self.render_mode {
            SessionRenderMode::Full => self.render_layout(window, cx).into_any_element(),
            SessionRenderMode::Sidebar => self.render_sidebar(p, window, cx).into_any_element(),
            SessionRenderMode::Main => self.render_main_area(p, cx).into_any_element(),
        }
    }
}
