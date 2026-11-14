use aries_communicate::Notifier;

use crate::session::{Session, SessionError};

impl Session {
    pub async fn set_session_mode(&mut self, mode: impl AsRef<str>) -> Result<(), SessionError> {
        let mode = mode.as_ref();
        let agent = self
            .extensions
            .agents
            .iter()
            .find(|agent| agent.frontmatter.name.eq_ignore_ascii_case(mode))
            .cloned()
            .ok_or_else(|| SessionError::agent_not_found(mode))?;

        self.state.agent(agent);

        // 不同的模式, 注册的工具是不同的，所以先删除全部内置的，然后再重新添加
        for tool_name in crate::tools::ALL_TOOL_NAMES {
            self.tool_server_handle.remove_tool(tool_name);
        }
        self.agent = crate::agent::Builder::new(
            self.state.clone(),
            self.extensions.clone(),
            self.tool_server_handle.clone(),
            self.gctx.clone(),
            Notifier::clone(&self.notifier),
            self.db.clone(),
        )
        .build(self.hook.clone())
        .await;

        Ok(())
    }
}
