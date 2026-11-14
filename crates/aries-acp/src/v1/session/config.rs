use agent_client_protocol::v1::schema::{
    SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    SessionConfigSelectOptions, SessionMode, SessionModeId, SessionModeState,
};
use aries_extension::agent::AgentDefinition;
use aries_init::Setting;
use aries_runtime::session::SessionConfig;
use rig::completion::Effort;

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
        SessionConfig::Mode.to_string(),
        SessionConfig::Mode.to_string(),
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
        SessionConfig::Model.to_string(),
        SessionConfig::Model.to_string(),
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
        SessionConfig::ThoughtLevel.to_string(),
        SessionConfig::ThoughtLevel.to_string(),
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
