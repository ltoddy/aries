use crate::workspace::session::session_view::*;

impl SessionView {
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
            self.directory().to_owned(),
            self.execution.conversation().title(),
            self.execution.loading_id().cloned(),
        )
    }
    pub fn sync_sidebar_active(
        &mut self,
        snapshot: Option<(Option<SessionId>, PathBuf, String, Option<SessionId>)>,
        cx: &mut Context<Self>,
    ) {
        let (id, cwd, mut title, loading) = match snapshot {
            Some((id, cwd, title, loading)) => (id, Some(cwd), Some(title), loading),
            None => (None, None, None, None),
        };
        if let Some(actual) = id.as_ref().and_then(|id| self.sidebar.list().confirmed_title(id)) {
            title = Some(actual.to_owned());
        }
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
        self.sidebar.locate_current_session(
            self.execution.session_id(),
            self.execution.directory(),
            window,
            cx,
        );
    }
    pub fn session_ids(&self, cwd: Option<&Path>) -> Vec<SessionId> {
        let mut ids = self.sidebar.motion().session_ids(cwd);
        if !self.sidebar.list().show_archived()
            && cwd.is_none_or(|cwd| cwd == self.directory())
            && let Some(id) = self.execution.session_id()
            && !self.sidebar.list().is_archived(id)
            && !ids.contains(id)
        {
            ids.push(id.clone());
        }
        ids.retain(|id| !self.sidebar.motion().is_deleted(id));
        ids
    }
    pub fn old_session_ids(&self, cwd: impl AsRef<Path>, days: u16) -> Vec<SessionId> {
        let cwd = cwd.as_ref();
        let now = Timestamp::now();
        let age = SignedDuration::from_hours(i64::from(days) * 24);
        self.session_ids(Some(cwd))
            .into_iter()
            .filter(|id| {
                self.sidebar
                    .list()
                    .session(id)
                    .and_then(|session| session.updated_at.as_deref())
                    .and_then(|time| time.parse::<Timestamp>().ok())
                    .is_some_and(|time| now.duration_since(time) > age)
            })
            .collect()
    }
    pub fn delete_old_sessions(
        &mut self,
        cwd: impl AsRef<Path>,
        days: u16,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_manage_sessions() || !self.supports_session_deletion() {
            return;
        }
        let cwd = cwd.as_ref();
        let ids = self.old_session_ids(cwd, days);
        if ids.is_empty() {
            return;
        }
        let count = ids.len();
        let anchor = format!("project-prune:{}", cwd.display());
        self.prepare_session_deletion(ids, anchor.clone(), cx);
        let directory = cwd.file_name().unwrap_or(cwd.as_os_str()).to_string_lossy();
        let title = format!("删除「{directory}」的旧会话？");
        let description = format!(
            "将删除项目 {} 中最后更新时间超过 {days} 天的 {count} 个会话，此操作无法撤销。",
            cwd.display()
        );
        let view = cx.weak_entity();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let confirm_view = view.clone();
            let close_view = view.clone();
            let confirm_anchor = anchor.clone();
            let close_anchor = anchor.clone();
            dialog
                .title(title.clone())
                .description(description.clone())
                .confirm()
                .ok_text("删除")
                .ok_variant(ButtonVariant::Danger)
                .cancel_text("取消")
                .on_ok(move |_, _, cx| {
                    let _ = confirm_view.update(cx, |this, cx| {
                        if this
                            .deletion
                            .pending_deletion()
                            .is_some_and(|pending| pending.is_anchor(&confirm_anchor))
                        {
                            this.confirm_session_deletion(cx);
                        }
                    });
                    true
                })
                .on_close(move |_, _, cx| {
                    let _ = close_view.update(cx, |this, cx| {
                        if this
                            .deletion
                            .pending_deletion()
                            .is_some_and(|pending| pending.is_anchor(&close_anchor))
                        {
                            this.cancel_session_deletion(cx);
                        }
                    });
                })
        });
    }
    pub fn prepare_session_deletion(
        &mut self,
        ids: impl IntoIterator<Item = SessionId>,
        anchor: String,
        cx: &mut Context<Self>,
    ) {
        if !self.can_manage_sessions() || !self.supports_session_deletion() {
            return;
        }
        let ids: Vec<_> = ids.into_iter().collect();
        if self.tab_host().and_then(|host| host.upgrade()).is_some_and(|host| {
            ids.iter().any(|id| !host.read(cx).session_management_available(id, cx))
        }) {
            return;
        }
        if self.deletion.prepare_available(ids, anchor, self.session_ids(None)) {
            cx.notify();
        }
    }

    pub fn confirm_session_deletion(&mut self, cx: &mut Context<Self>) {
        let blocked = self.execution.is_disconnected()
            || self.execution.conversation().is_busy()
            || self.sidebar.list().is_loading()
            || self.management.is_pending()
            || !self.supports_session_deletion();
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
}
