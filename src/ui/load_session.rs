use agent_client_protocol::Error;
use agent_client_protocol::v1::schema::{
    LoadSessionRequest, LoadSessionResponse, SessionId, SessionInfo,
};
use gpui_kit::*;

use crate::acp::Command;
use crate::session::load::SessionLoad;
use crate::ui::ChatView;

impl ChatView {
    pub fn can_load_sessions(&self) -> bool {
        self.load_supported
            && !self.disconnected
            && self.is_session_idle()
            && !self.session_list.is_loading()
    }

    pub fn load_session(&mut self, session: SessionInfo, cx: &mut Context<Self>) {
        if !self.can_load_sessions() || self.session_id.as_ref() == Some(&session.session_id) {
            return;
        }
        let request = LoadSessionRequest::new(session.session_id.clone(), session.cwd.clone());
        if !self
            .bridge
            .as_ref()
            .is_some_and(|bridge| bridge.send(Command::LoadSession(Box::new(request))).is_ok())
        {
            self.status = "加载失败".into();
            self.chat.finish(Some("无法发送加载请求，请重新连接后重试。".into()));
            cx.notify();
            return;
        }
        self.dismiss_permissions();
        self.chat.finish(None);
        self.loading_session = Some(SessionLoad::new(session));
        self.status = "正在加载会话…".into();
        cx.notify();
    }

    pub fn finish_session_load(
        &mut self,
        id: SessionId,
        result: Result<LoadSessionResponse, Error>,
        window: &mut Window,
        cx: &mut Context<Self>,
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
                let (session, mut chat, tokens) = loading.into_parts();
                chat.finish(None);
                if let Some(options) = response.config_options {
                    chat.set_config_options(options);
                }
                self.save_draft(cx);
                let draft = self.drafts.remove(&session.session_id).unwrap_or_default();
                self.reset_chat();
                self.chat = chat;
                self.cwd = session.cwd;
                self.watch_git_branch(cx);
                self.session_id = Some(session.session_id);
                self.tokens = tokens;
                self.input.update(cx, |input, cx| input.set_value(draft, window, cx));
                self.rebuild_images();
                self.rebuild_tool_diffs(cx);
                self.timeline.reset_with_uniform_height(self.chat.messages().len(), px(96.));
                self.timeline.scroll_to_end();
            }
            Err(error) => {
                self.status = "加载失败".into();
                self.chat.finish(Some(format!("无法加载会话：{error}")));
            }
        }
        if self.session_list.take_refresh_request() {
            self.refresh_sessions(cx);
        }
        cx.notify();
    }
}
