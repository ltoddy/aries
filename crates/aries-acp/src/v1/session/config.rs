use std::fmt::{Display, Formatter};
use std::str::FromStr;

use agent_client_protocol::v1::schema::{
    SessionConfigId, SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    SessionConfigSelectOptions,
};
use aries_extension::AgentDefinition;
use aries_init::Setting;

#[derive(Debug, Copy, Clone)]
pub enum SessionConfig {
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
pub struct ParseSessionConfigError;

impl Display for ParseSessionConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        "provided string was not `mode` or `model`".fmt(f)
    }
}

impl std::error::Error for ParseSessionConfigError {}

impl FromStr for SessionConfig {
    type Err = ParseSessionConfigError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        match s {
            "mode" => Ok(SessionConfig::Mode),
            "model" => Ok(SessionConfig::Model),
            _ => Err(ParseSessionConfigError),
        }
    }
}

pub fn config_options(
    setting: &Setting,
    current_mode: String,
    agents: &[AgentDefinition],
) -> Vec<SessionConfigOption> {
    vec![mode_option(current_mode, agents), model_option(setting)]
}

fn mode_option(current: String, agents: &[AgentDefinition]) -> SessionConfigOption {
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
