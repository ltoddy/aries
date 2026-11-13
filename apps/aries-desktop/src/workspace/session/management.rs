use agent_client_protocol::v1::schema::{
    ForkSessionRequest, SessionCapabilities, SessionId, SessionInfo, SetSessionTitleRequest,
};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{ActiveTheme, Disableable, WindowExt};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::acp::{Command, Event};
use crate::workspace::session::session_view::SessionView;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SessionOperation {
    Title,
    Fork,
    Archive(bool),
}

impl SessionOperation {
    pub fn title() -> Self {
        Self::Title
    }
    pub fn fork() -> Self {
        Self::Fork
    }
    pub fn archive(archived: bool) -> Self {
        Self::Archive(archived)
    }
}

pub struct RenameSession {
    session: SessionInfo,
    input: Entity<InputState>,
    error: Option<String>,
    _subscription: Subscription,
}

impl RenameSession {
    fn new(session: SessionInfo, input: Entity<InputState>, subscription: Subscription) -> Self {
        Self { session, input, error: None, _subscription: subscription }
    }
}

pub struct SessionManagement {
    capabilities: SessionCapabilities,
    pending: Option<(SessionInfo, SessionOperation)>,
    rename: Option<RenameSession>,
    error: Option<String>,
    locked: bool,
}

impl SessionManagement {
    pub fn new() -> Self {
        Self {
            capabilities: SessionCapabilities::new(),
            pending: None,
            rename: None,
            error: None,
            locked: false,
        }
    }
    pub fn has_local_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn is_locked(&self) -> bool {
        self.locked
    }
    pub fn is_pending(&self) -> bool {
        self.pending.is_some() || self.locked
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn capabilities(&self) -> &SessionCapabilities {
        &self.capabilities
    }
}

impl SessionView {
    pub fn management(&self) -> &SessionManagement {
        &self.management
    }
    pub fn set_management_lock(&mut self, locked: bool, cx: &mut Context<Self>) {
        self.management.locked = locked;
        if !locked && self.sidebar.list_mut().take_refresh_request() {
            self.refresh_sessions(cx);
        }
        cx.notify();
    }
    pub fn supports_session_deletion(&self) -> bool {
        self.management.capabilities.delete.is_some()
    }
    pub fn session_management_capabilities(
        &self,
        session: &SessionId,
        cx: &App,
    ) -> SessionCapabilities {
        if self.uses_sidebar_tab_host()
            && let Some(host) = self.tab_host().and_then(|host| host.upgrade())
            && let Some(source) = host.read(cx).session_management_source(session, cx)
        {
            return source.read(cx).management().capabilities().clone();
        }
        self.management.capabilities.clone()
    }

    pub fn fail_session_management(&mut self, id: &SessionId, cx: &mut Context<Self>) {
        if !self.management.pending.as_ref().is_some_and(|(session, _)| &session.session_id == id) {
            return;
        }
        self.management.pending = None;
        self.release_management(id, cx);
        self.management.error = Some("连接已断开，会话操作结果未确认。".into());
        if let Some(rename) = &mut self.management.rename {
            rename.error = self.management.error.clone();
        }
        cx.notify();
    }

    pub fn reload_archived_session(
        &mut self,
        session: SessionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.coordinate(|execution, parts| execution.save_draft(parts, cx));
        self.coordinate(|execution, parts| execution.reset_chat(parts));
        self.input().update(cx, |input, cx| input.set_value("", window, cx));
        self.queue_session_load(session, cx);
    }

    pub fn management_available(&self, session: &SessionId, cx: &App) -> bool {
        self.can_manage_sessions()
            && self
                .tab_host()
                .and_then(|host| host.upgrade())
                .is_none_or(|host| host.read(cx).session_management_available(session, cx))
    }
    fn begin_management(
        &mut self,
        session: SessionInfo,
        operation: SessionOperation,
        command: Command,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.management_available(&session.session_id, cx) {
            self.management.error = Some("会话正在处理中，请稍后重试。".into());
            cx.notify();
            return false;
        }
        let source = self
            .uses_sidebar_tab_host()
            .then(|| {
                self.tab_host().and_then(|host| host.upgrade()).and_then(|host| {
                    host.read(cx).session_management_source(&session.session_id, cx).cloned()
                })
            })
            .flatten();
        let sent = if let Some(source) = source {
            source.read(cx).execution().bridge().is_some_and(|bridge| bridge.send(command).is_ok())
        } else {
            self.execution.bridge().is_some_and(|bridge| bridge.send(command).is_ok())
        };
        if !sent {
            self.management.error = Some("无法发送会话操作，请重新连接后重试。".into());
            cx.notify();
            return false;
        }
        if let Some(host) = self.tab_host() {
            let id = session.session_id.clone();
            let _ = host.update(cx, |host, cx| host.lock_session_management(&id, true, cx));
        }
        self.management.error = None;
        self.management.pending = Some((session, operation));
        cx.notify();
        true
    }
    pub fn archive_session(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        if self.session_management_capabilities(&session.session_id, cx).archive.is_none() {
            return;
        }
        let archived = !session.archived.unwrap_or(false);
        let command = if archived {
            Command::ArchiveSession(session.session_id.clone())
        } else {
            Command::UnarchiveSession(session.session_id.clone())
        };
        self.begin_management(session, SessionOperation::archive(archived), command, cx);
    }
    pub fn fork_session(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        if self.session_management_capabilities(&session.session_id, cx).fork.is_none() {
            return;
        }
        let request = ForkSessionRequest::new(session.session_id.clone(), session.cwd.clone());
        self.begin_management(
            session,
            SessionOperation::fork(),
            Command::ForkSession(Box::new(request)),
            cx,
        );
    }
    pub fn rename_session(
        &mut self,
        session: SessionInfo,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.session_management_capabilities(&session.session_id, cx).title.is_none()
            || !self.management_available(&session.session_id, cx)
        {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx));
        input.update(cx, |input, cx| {
            input.set_value(session.title.clone().unwrap_or_default(), window, cx)
        });
        let subscription = cx.subscribe_in(&input, window, |this, _, event, _, cx| match event {
            InputEvent::PressEnter { .. } => this.confirm_session_rename(cx),
            InputEvent::Change => {
                if let Some(rename) = &mut this.management.rename {
                    rename.error = None;
                }
                cx.notify();
            },
            _ => {},
        });
        self.management.rename = Some(RenameSession::new(session, input.clone(), subscription));
        let owner = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let Some(view) = owner.upgrade() else {
                return dialog;
            };
            let this = view.read(cx);
            let Some(rename) = &this.management.rename else {
                return dialog;
            };
            let pending = this.management.is_pending();
            let confirm = owner.clone();
            let cancel = owner.clone();
            let close = owner.clone();
            let ok = owner.clone();
            dialog
                .title("重命名会话")
                .w(cx.theme().font_size * 28.)
                .overlay_closable(false)
                .close_button(!pending)
                .on_ok(move |_, _, cx| {
                    let _ = ok.update(cx, |this, cx| this.confirm_session_rename(cx));
                    false
                })
                .on_cancel(move |_, _, cx| owner_cancel(&cancel, cx))
                .on_close(move |_, _, cx| {
                    let _ = close.update(cx, |this, cx| {
                        this.management.rename = None;
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child("会话名称")
                        .child(Input::new(&rename.input).disabled(pending))
                        .when_some(rename.error.clone(), |form, error| {
                            form.child(div().text_sm().text_color(cx.theme().danger).child(error))
                        }),
                )
                .footer(
                    div()
                        .flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("cancel-session-rename")
                                .label("取消")
                                .disabled(pending)
                                .on_click({
                                    let cancel = owner.clone();
                                    move |_, window, cx| {
                                        if owner_cancel(&cancel, cx) {
                                            window.close_dialog(cx);
                                        }
                                    }
                                }),
                        )
                        .child(
                            Button::new("confirm-session-rename")
                                .primary()
                                .label(if pending { "保存中" } else { "保存" })
                                .disabled(
                                    pending || rename.input.read(cx).value().trim().is_empty(),
                                )
                                .on_click(move |_, _, cx| {
                                    let _ = confirm
                                        .update(cx, |this, cx| this.confirm_session_rename(cx));
                                }),
                        ),
                )
        });
        window.on_next_frame(move |window, cx| {
            input.update(cx, |input, cx| input.focus(window, cx));
        });
        cx.notify();
    }
    fn confirm_session_rename(&mut self, cx: &mut Context<Self>) {
        if self.management.is_pending() {
            return;
        }
        let Some(rename) = &mut self.management.rename else {
            return;
        };
        let title = rename.input.read(cx).value().trim().to_owned();
        if title.is_empty() {
            rename.error = Some("会话名称不能为空".into());
            cx.notify();
            return;
        }
        let session = rename.session.clone();
        if self.session_management_capabilities(&session.session_id, cx).title.is_none() {
            return;
        }
        let request = SetSessionTitleRequest::new(session.session_id.clone(), title);
        if !self.begin_management(
            session,
            SessionOperation::title(),
            Command::SetSessionTitle(Box::new(request)),
            cx,
        ) && let Some(rename) = &mut self.management.rename
        {
            rename.error = Some(
                self.management
                    .error
                    .clone()
                    .unwrap_or_else(|| "会话正在处理中，请稍后重试。".into()),
            );
            cx.notify();
        }
    }
    pub fn sync_session_title(&mut self, id: &SessionId, title: String, cx: &mut Context<Self>) {
        self.sidebar.list_mut().set_title(id, title.clone());
        self.sidebar.motion_mut().set_title(id, &title);
        self.execution.set_session_title(id, title);
        self.sync_sidebar_rows(cx);
        cx.notify();
    }
    pub fn apply_management_event(
        &mut self,
        event: &Event,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Event::Connected { session_capabilities, .. } = event {
            self.management.capabilities = session_capabilities.as_ref().clone();
            return false;
        }
        if let Event::Disconnected(_) = event {
            self.management.capabilities = SessionCapabilities::new();
            self.management.locked = false;
            if let Some((session, _)) = self.management.pending.take() {
                self.release_management(&session.session_id, cx);
                self.management.error = Some("连接已断开，会话操作结果未确认。".into());
                if let Some(rename) = &mut self.management.rename {
                    rename.error = self.management.error.clone();
                }
            }
            return false;
        }
        let (id, operation) = match event {
            Event::SessionTitleSet { session_id, .. } => (session_id, SessionOperation::title()),
            Event::SessionArchived { session_id, archived, .. } => {
                (session_id, SessionOperation::archive(*archived))
            },
            Event::SessionForked { session_id, .. } => (session_id, SessionOperation::fork()),
            _ => return false,
        };
        if !self
            .management
            .pending
            .as_ref()
            .is_some_and(|(session, pending)| &session.session_id == id && *pending == operation)
        {
            return true;
        }
        let Some((session, _)) = self.management.pending.take() else {
            return true;
        };
        self.release_management(id, cx);
        let error = match event {
            Event::SessionTitleSet { result, .. } => match result {
                Ok(title) => {
                    self.sync_session_title(id, title.clone(), cx);
                    if let Some(host) = self.tab_host() {
                        let id = id.clone();
                        let title = title.clone();
                        cx.defer(move |cx| {
                            let _ =
                                host.update(cx, |host, cx| host.sync_session_title(&id, title, cx));
                        });
                    }
                    self.management.rename = None;
                    window.close_dialog(cx);
                    None
                },
                Err(error) => Some(crate::error::details(error)),
            },
            Event::SessionArchived { result, archived, .. } => match result {
                Ok(()) => {
                    self.sidebar.list_mut().set_archived(id, *archived);
                    self.sidebar.motion_mut().remove_session(id);
                    if *archived {
                        if let Some(host) = self.tab_host() {
                            let id = id.clone();
                            cx.defer(move |cx| {
                                let _ = host
                                    .update(cx, |host, cx| host.close_archived_session(&id, cx));
                            });
                        } else if self.matches_session(id) {
                            self.coordinate(|execution, parts| execution.save_draft(parts, cx));
                            self.coordinate(|execution, parts| execution.reset_chat(parts));
                            self.input().update(cx, |input, cx| input.set_value("", window, cx));
                        }
                    }
                    None
                },
                Err(error) => Some(crate::error::details(error)),
            },
            Event::SessionForked { result, .. } => match result {
                Ok(response) => {
                    let supports_load = if self.uses_sidebar_tab_host() {
                        self.tab_host()
                            .and_then(|host| host.upgrade())
                            .and_then(|host| {
                                host.read(cx)
                                    .session_management_source(id, cx)
                                    .map(|source| source.read(cx).execution().supports_load())
                            })
                            .unwrap_or(self.execution.supports_load())
                    } else {
                        self.execution.supports_load()
                    };
                    let fork = SessionInfo::new(response.session_id.clone(), session.cwd)
                        .title(session.title);
                    if supports_load {
                        if self.tab_host().and_then(|host| host.upgrade()).is_some() {
                            self.request_session_tab(fork, cx);
                        } else {
                            self.load_session(fork, cx);
                        }
                    } else {
                        window.push_notification(
                            Notification::info(format!(
                                "已复制会话 {}，Agent 不支持自动加载。",
                                response.session_id
                            )),
                            cx,
                        );
                    }
                    None
                },
                Err(error) => Some(crate::error::details(error)),
            },
            _ => None,
        };
        self.management.error = error.clone();
        if let Some(error) = error {
            if let Some(rename) = &mut self.management.rename {
                rename.error = Some(error.clone());
            }
            window.push_notification(Notification::error(format!("会话操作未完成：{error}")), cx);
        }
        self.refresh_sessions(cx);
        self.sync_sidebar_rows(cx);
        cx.notify();
        true
    }
    fn release_management(&self, id: &SessionId, cx: &mut Context<Self>) {
        if let Some(host) = self.tab_host() {
            let id = id.clone();
            cx.defer(move |cx| {
                let _ = host.update(cx, |host, cx| host.lock_session_management(&id, false, cx));
            });
        }
    }
}

fn owner_cancel(owner: &WeakEntity<SessionView>, cx: &mut App) -> bool {
    owner.upgrade().is_none_or(|view| {
        view.update(cx, |this, cx| {
            if this.management.is_pending() {
                return false;
            }
            this.management.rename = None;
            cx.notify();
            true
        })
    })
}
