use agent_client_protocol::Error;
use agent_client_protocol::v1::schema::{
    SessionConfigId, SessionConfigOption, SessionId, SetSessionConfigOptionRequest,
};
use gpui_kit::*;

use crate::acp::Command;
use crate::workspace::SessionView;
use crate::workspace::session::execution::Execution;
use crate::workspace::session::model::config;

#[derive(Default)]
pub struct Configuration {
    pending: Option<SessionConfigId>,
    error: Option<String>,
}
impl Configuration {
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub fn reset(&mut self) {
        self.pending = None;
        self.error = None;
    }
    pub fn disconnect(&mut self) {
        if self.pending.take().is_some() {
            self.error = Some("连接已断开，无法确认配置是否更新。".into());
        }
    }
    pub fn set_session_config(
        &mut self,
        execution: &Execution,
        idle: bool,
        request: SetSessionConfigOptionRequest,
        cx: &mut Context<SessionView>,
    ) {
        if execution.session_id() != Some(&request.session_id)
            || !idle
            || execution.is_disconnected()
            || !execution.conversation().config_options().iter().any(|option| {
                option.id == request.config_id && config::can_select(option, &request.value)
            })
        {
            return;
        }
        let config_id = request.config_id.clone();
        self.error = None;
        if execution.bridge().is_some_and(|bridge| {
            bridge.send(Command::SetSessionConfigOption(Box::new(request))).is_ok()
        }) {
            self.pending = Some(config_id);
        } else {
            self.error = Some("无法更新会话配置，请重新连接后重试。".into());
        }
        cx.notify();
    }

    pub fn finish_session_config(
        &mut self,
        execution: &mut Execution,
        session_id: &SessionId,
        config_id: &SessionConfigId,
        result: Result<Vec<SessionConfigOption>, Error>,
        cx: &mut Context<SessionView>,
    ) {
        if execution.session_id() != Some(session_id) || self.pending.as_ref() != Some(config_id) {
            return;
        }
        self.pending = None;
        match result {
            Ok(options) => {
                execution.set_config_options(options);
                self.error = None;
            },
            Err(error) => self.error = Some(format!("无法更新会话配置：{error}")),
        }
        cx.notify();
    }
}
