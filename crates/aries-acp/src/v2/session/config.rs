use std::fmt::{Display, Formatter};
use std::str::FromStr;

use agent_client_protocol::schema::v2::{
    SessionConfigId, SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    SessionConfigSelectOptions,
};
use aries_init::Setting;
use aries_mode::Mode;

#[derive(Debug, Copy, Clone)]
pub(super) enum SessionConfig {
    Mode,
    Model,
}

impl Display for SessionConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionConfig::Mode => write!(f, "mode"),
            SessionConfig::Model => write!(f, "model"),
        }
    }
}

impl From<SessionConfig> for SessionConfigId {
    fn from(val: SessionConfig) -> Self {
        match val {
            SessionConfig::Mode => SessionConfigId::new("mode"),
            SessionConfig::Model => SessionConfigId::new("model"),
        }
    }
}

impl From<SessionConfig> for String {
    fn from(val: SessionConfig) -> Self {
        match val {
            SessionConfig::Mode => String::from("mode"),
            SessionConfig::Model => String::from("model"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ParseSessionConfigError;

impl Display for ParseSessionConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        "provided string was not `mode` or `model`".fmt(f)
    }
}

impl std::error::Error for ParseSessionConfigError {}

impl FromStr for SessionConfig {
    type Err = ParseSessionConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "mode" => Ok(SessionConfig::Mode),
            "model" => Ok(SessionConfig::Model),
            _ => Err(ParseSessionConfigError),
        }
    }
}

pub(super) fn config_options(setting: &Setting, current_mode: Mode) -> Vec<SessionConfigOption> {
    vec![mode_option(current_mode), model_option(setting)]
}

fn mode_option(current: Mode) -> SessionConfigOption {
    let options = [Mode::Build, Mode::Plan, Mode::General, Mode::Explore]
        .into_iter()
        .map(|agent| {
            SessionConfigSelectOption::new(agent.id(), agent.name())
                .description(Some(agent.description().to_owned()))
        })
        .collect::<Vec<_>>();

    SessionConfigOption::select(
        SessionConfig::Mode,
        SessionConfig::Mode,
        current.id(),
        SessionConfigSelectOptions::Ungrouped(options),
    )
    .description("Agent mode determines how Aries processes your requests — Build (coding), Plan (no edits), General (multi-step), Explore (codebase search).")
    .category(SessionConfigOptionCategory::Mode)
}

fn model_option(setting: &Setting) -> SessionConfigOption {
    let options = setting
        .models
        .iter()
        .map(|m| {
            let alias = m.alias();
            SessionConfigSelectOption::new(alias.clone(), alias)
        })
        .collect::<Vec<_>>();

    SessionConfigOption::select(
        SessionConfig::Model,
        SessionConfig::Model,
        setting.active.clone(),
        SessionConfigSelectOptions::Ungrouped(options),
    )
    .description("The language model that powers this session. Switch models to change providers, capabilities, or context window size.")
    .category(SessionConfigOptionCategory::Model)
}
