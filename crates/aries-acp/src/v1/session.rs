use std::fmt::{Display, Formatter};
use std::str::FromStr;

use agent_client_protocol::schema::v1::{
    CloseSessionRequest, CloseSessionResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
    ResumeSessionRequest, ResumeSessionResponse, SessionConfigId, SessionConfigOption,
    SessionConfigOptionCategory, SessionConfigSelectOption, SessionConfigSelectOptions,
    SessionInfo, SetSessionConfigOptionRequest, SetSessionConfigOptionResponse,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use aries_core::agents::Mode;
use aries_init::Setting;
use tracing::info;

use super::SharedRegistry;
use crate::v1::mcp::McpServers;

pub async fn new_session(
    req: NewSessionRequest,
    responder: Responder<NewSessionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received new session request {req:?}");

    let mcp_servers = McpServers(req.mcp_servers);
    let mcp_config = mcp_servers.into();
    let mut registry = registry.lock().await;
    let cwd = req.cwd.display().to_string();
    let session = match registry.new_session(cwd, mcp_config).await {
        Ok(session) => session,
        Err(err) => {
            return responder.respond_with_internal_error(err.to_string());
        },
    };

    let config_options = config_options(session.setting(), session.mode());
    let resp = NewSessionResponse::new(session.id()).config_options(config_options);
    responder.respond(resp)
}

pub async fn load_session(
    req: LoadSessionRequest,
    responder: Responder<LoadSessionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received list sessions request {req:?}");

    let session_id = req.session_id.to_string();
    let mut registry = registry.lock().await;

    let mcp_servers = McpServers(req.mcp_servers);
    let mcp_config = mcp_servers.into();
    let session = match registry.load_session(session_id, mcp_config).await {
        Ok(session) => session,
        Err(err) => return responder.respond_with_internal_error(err.to_string()),
    };

    let config_options = config_options(session.setting(), session.mode());
    let resp = LoadSessionResponse::new().config_options(config_options);
    responder.respond(resp)
}

pub async fn list_session(
    req: ListSessionsRequest,
    responder: Responder<ListSessionsResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received list sessions request {req:?}");

    let mut registry = registry.lock().await;
    let sessions = match registry.list_sessions(req.cwd).await {
        Ok(sessions) => sessions,
        Err(err) => {
            return responder.respond_with_internal_error(err.to_string());
        },
    };
    let sessions = sessions
        .into_iter()
        .map(|s| {
            SessionInfo::new(s.session_id, s.cwd)
                .title(s.title)
                .updated_at(s.updated_at.to_string())
        })
        .collect();

    let resp = ListSessionsResponse::new(sessions);
    responder.respond(resp)
}

pub async fn close_session(
    req: CloseSessionRequest,
    responder: Responder<CloseSessionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    let session_id = req.session_id.to_string();
    info!("Received close session request for {session_id}");

    let mut registry = registry.lock().await;
    registry.close_session(session_id).await;

    let resp = CloseSessionResponse::new();
    responder.respond(resp)
}

pub async fn set_session_config_option(
    req: SetSessionConfigOptionRequest,
    responder: Responder<SetSessionConfigOptionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received set session config option request {req:?}");

    let session_id = req.session_id.to_string();
    let config_id = req.config_id.to_string();
    let value = req.value.as_value_id().map(|v| v.to_string()).unwrap_or_default();

    let mut session = {
        let registry = registry.lock().await;
        match registry.get_session(&session_id) {
            Some(session) => session,
            None => {
                return responder.respond_with_error(Error::resource_not_found(Some(session_id)));
            },
        }
    };

    if let Ok(session_config) = config_id.parse::<SessionConfig>() {
        return match session_config {
            SessionConfig::Mode => {
                let mode = Mode::from_str(&value).unwrap_or_default();
                if let Err(err) = session.set_mode(mode).await {
                    return responder.respond_with_internal_error(err.to_string());
                }
                let resp = SetSessionConfigOptionResponse::new(config_options(
                    session.setting(),
                    session.mode(),
                ));
                responder.respond(resp)
            },
            SessionConfig::Model => {
                if let Err(err) = session.set_model(value).await {
                    return responder.respond_with_internal_error(err.to_string());
                };
                let resp = SetSessionConfigOptionResponse::new(config_options(
                    session.setting(),
                    session.mode(),
                ));
                responder.respond(resp)
            },
        };
    }

    let resp =
        SetSessionConfigOptionResponse::new(config_options(session.setting(), session.mode()));
    responder.respond(resp)
}

pub async fn resume_session(
    req: ResumeSessionRequest,
    responder: Responder<ResumeSessionResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received resume session request {req:?}");

    let resp = ResumeSessionResponse::new();
    responder.respond(resp)
}

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
        "provieded string was not `mode` or `model`".fmt(f)
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

fn config_options(setting: &Setting, current_mode: Mode) -> Vec<SessionConfigOption> {
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
