use std::fmt::{Display, Formatter};
use std::str::FromStr;

use agent_client_protocol::v1::schema::{
    SessionConfigId, SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    SessionConfigSelectOptions, SessionMode, SessionModeId, SessionModeState,
};
use aries_extension::agent::AgentDefinition;
use aries_init::Setting;
use rig::completion::Effort;

#[derive(Debug, Copy, Clone)]
pub enum SessionConfig {
    Mode,
    Model,
    ThoughtLevel,
}

impl Display for SessionConfig {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionConfig::Mode => write!(f, "mode"),
            SessionConfig::Model => write!(f, "model"),
            SessionConfig::ThoughtLevel => write!(f, "thought level"),
        }
    }
}

impl From<SessionConfig> for SessionConfigId {
    fn from(val: SessionConfig) -> Self {
        match val {
            SessionConfig::Mode => SessionConfigId::new("mode"),
            SessionConfig::Model => SessionConfigId::new("model"),
            SessionConfig::ThoughtLevel => SessionConfigId::new("thought level"),
        }
    }
}

impl From<SessionConfig> for String {
    fn from(val: SessionConfig) -> Self {
        match val {
            SessionConfig::Mode => String::from("mode"),
            SessionConfig::Model => String::from("model"),
            SessionConfig::ThoughtLevel => String::from("thought level"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid session config ID `{0}`: expected `mode`, `model`, or `thought level`")]
pub struct ParseSessionConfigError(String);

impl ParseSessionConfigError {
    pub fn new(config_id: impl Into<String>) -> Self {
        Self(config_id.into())
    }
}

impl FromStr for SessionConfig {
    type Err = ParseSessionConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "mode" => Ok(Self::Mode),
            "model" => Ok(Self::Model),
            "thought level" => Ok(Self::ThoughtLevel),
            _ => Err(ParseSessionConfigError::new(s)),
        }
    }
}

pub fn config_options(
    setting: &Setting,
    current_mode: impl Into<String>,
    agents: impl AsRef<[AgentDefinition]>,
    thought_level: Effort,
) -> Vec<SessionConfigOption> {
    vec![
        mode_option(current_mode, agents),
        model_option(setting),
        thought_level_option(thought_level),
    ]
}

fn mode_option(
    current: impl Into<String>,
    agents: impl AsRef<[AgentDefinition]>,
) -> SessionConfigOption {
    let current = current.into();
    let agents = agents.as_ref();
    let mut options: Vec<_> = vec![];

    for agent in agents {
        options.push(
            SessionConfigSelectOption::new(
                agent.frontmatter.name.clone(),
                agent.frontmatter.name.clone(),
            )
            .description(Some(agent.frontmatter.description.clone())),
        );
    }

    SessionConfigOption::select(
        SessionConfig::Mode,
        SessionConfig::Mode,
        current,
        SessionConfigSelectOptions::Ungrouped(options),
    )
    .description(
        "Agent mode determines how Aries processes your requests — configured via extensions.",
    )
    .category(SessionConfigOptionCategory::Mode)
}

fn model_option(setting: &Setting) -> SessionConfigOption {
    let options = setting
        .models
        .iter()
        .map(|m| {
            let alias = m.alias();
            SessionConfigSelectOption::new(alias.to_owned(), alias)
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

fn thought_level_option(current: Effort) -> SessionConfigOption {
    let efforts =
        [Effort::Minimal, Effort::Low, Effort::Medium, Effort::High, Effort::XHigh, Effort::Max];
    let options = efforts
        .iter()
        .map(|e| SessionConfigSelectOption::new(e.as_str(), e.as_str()))
        .collect::<Vec<_>>();

    SessionConfigOption::select(
        SessionConfig::ThoughtLevel,
        SessionConfig::ThoughtLevel,
        current.as_str(),
        options,
    )
    .description("")
    .category(SessionConfigOptionCategory::ThoughtLevel)
}

#[derive(Debug, thiserror::Error)]
#[error("session mode `{0}` not found")]
pub struct SessionModeError(String);

impl SessionModeError {
    pub fn new(mode: impl Into<String>) -> Self {
        Self(mode.into())
    }
}

pub fn modes(
    current: impl AsRef<str>,
    agents: impl AsRef<[AgentDefinition]>,
) -> Result<SessionModeState, SessionModeError> {
    let current = current.as_ref();
    let agents = agents.as_ref();
    let agent = agents
        .iter()
        .find(|agent| agent.frontmatter.name == current)
        .ok_or_else(|| SessionModeError::new(current))?;

    let available_modes = agents
        .iter()
        .map(|agent| {
            SessionMode::new(
                SessionModeId::new(agent.frontmatter.name.clone()),
                &agent.frontmatter.name,
            )
            .description(&agent.frontmatter.description)
        })
        .collect::<Vec<_>>();

    Ok(SessionModeState::new(SessionModeId::new(agent.frontmatter.name.clone()), available_modes))
}
