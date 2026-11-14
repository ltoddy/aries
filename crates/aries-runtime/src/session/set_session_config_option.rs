use std::str::FromStr;

use aries_communicate::Notifier;
use aries_init::SettingLoader;
use rig::completion::Effort;

use crate::session::{Session, SessionConfig, SessionError};

impl Session {
    pub async fn set_session_config_option(
        &mut self,
        config_id: impl AsRef<str>,
        value: impl AsRef<str>,
    ) -> Result<(), SessionError> {
        let config_id = config_id.as_ref();
        let value = value.as_ref();
        let config = SessionConfig::from_str(config_id).map_err(SessionError::parse_config)?;

        match config {
            SessionConfig::Mode => self.set_session_mode(value).await?,
            SessionConfig::Model => self.set_session_model(value).await?,
            SessionConfig::ThoughtLevel => self.set_session_thought_level(value).await?,
        };

        Ok(())
    }

    async fn set_session_model(&mut self, alias: impl AsRef<str>) -> Result<(), SessionError> {
        let alias = alias.as_ref();

        let mut setting = self.setting.clone();
        let config = setting.activate(alias).map_err(SessionError::setting)?;
        let model = crate::create_model(&config)?;

        let loader = SettingLoader::new(self.gctx.root_dir());
        loader.save(&setting).await.map_err(SessionError::setting)?;

        self.setting = setting;
        self.state.model(model);
        self.agent.set_model(self.state.model.clone());

        self.config = config;
        self.compactor.set_model(self.state.model.clone());

        Ok(())
    }

    async fn set_session_thought_level(
        &mut self,
        level: impl AsRef<str>,
    ) -> Result<(), SessionError> {
        let level = level.as_ref();
        let effort = match level {
            "minimal" => Effort::Minimal,
            "low" => Effort::Low,
            "medium" => Effort::Medium,
            "high" => Effort::High,
            "xhigh" => Effort::XHigh,
            "max" => Effort::Max,
            _ => return Err(SessionError::invalid_thought_level(level)),
        };

        self.state.effort(effort);

        let agent = crate::agent::Builder::new(
            self.state.clone(),
            self.extensions.clone(),
            self.tool_server_handle.clone(),
            self.gctx.clone(),
            Notifier::clone(&self.notifier),
            self.db.clone(),
        )
        .build(self.hook.clone())
        .await;
        self.agent = agent;

        Ok(())
    }
}
