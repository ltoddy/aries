use gpui_kit::*;

use crate::acp::Command;
use crate::chat::view::ChatView;

impl ChatView {
    pub fn refresh_sessions(&mut self, cx: &mut Context<Self>) {
        if self.disconnected || !self.session_list.is_supported() {
            return;
        }
        if self.session_list.is_loading()
            || self.session_deletion.is_busy()
            || self.loading_session.is_some()
        {
            self.session_list.defer_refresh();
            return;
        }
        if !self.session_list.begin_refresh() {
            return;
        }
        if !self.bridge.as_ref().is_some_and(|bridge| bridge.send(Command::ListSessions).is_ok()) {
            self.session_list.fail_refresh("无法获取会话列表，请新建对话重新连接 Aries。".into());
        }
        cx.notify();
    }
}
