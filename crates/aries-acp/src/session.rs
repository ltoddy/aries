use agent_client_protocol::schema::{
    CloseSessionRequest, CloseSessionResponse, ListSessionsRequest, ListSessionsResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse,
    ResumeSessionRequest, ResumeSessionResponse, SessionConfigOption, SessionConfigOptionCategory,
    SessionConfigSelectOption, SessionConfigSelectOptions, SessionInfo,
    SetSessionConfigOptionRequest, SetSessionConfigOptionResponse, SetSessionModeRequest,
    SetSessionModeResponse,
};
use agent_client_protocol::{Client, ConnectionTo, Error, Responder};
use aries_core::agents::AgentType;
use aries_init::Setting;
use tracing::info;

use crate::SharedRegistry;

pub async fn new_session(
    req: NewSessionRequest,
    responder: Responder<NewSessionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
    setting: Setting,
) -> Result<(), Error> {
    info!("Received new session request {req:?}");

    let mut reg = registry.lock().await;
    let cwd = req.cwd.display().to_string();
    let session = match reg.new_session(cwd).await {
        Ok(session) => session,
        Err(err) => {
            return responder.respond_with_internal_error(err.to_string());
        },
    };

    let config_options = config_options(&setting, AgentType::Build);
    let resp = NewSessionResponse::new(session.id()).config_options(config_options);
    responder.respond(resp)
}

pub async fn load_session(
    req: LoadSessionRequest,
    responder: Responder<LoadSessionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
    setting: Setting,
) -> Result<(), Error> {
    info!("Received list sessions request {req:?}");

    let session_id = req.session_id.to_string();
    let mut reg = registry.lock().await;
    if let Err(err) = reg.load_session(&session_id).await {
        return responder.respond_with_internal_error(err.to_string());
    }

    let config_options = config_options(&setting, AgentType::Build);
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

    let mut reg = registry.lock().await;
    let sessions = match reg.list_sessions(req.cwd).await {
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

    responder.respond(ListSessionsResponse::new(sessions))
}

pub async fn set_session_mode(
    req: SetSessionModeRequest,
    responder: Responder<SetSessionModeResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
) -> Result<(), Error> {
    info!("Received set session mode request {req:?}");

    let session_id = req.session_id.to_string();
    let mut session = {
        let reg = registry.lock().await;
        match reg.get_session(&session_id) {
            Some(session) => session,
            None => {
                return responder.respond_with_error(Error::resource_not_found(Some(session_id)));
            },
        }
    };

    let mode_id = req.mode_id.to_string();
    let agent_type = AgentType::from_id(&mode_id);
    if let Err(err) = session.switch_agent(agent_type).await {
        return responder.respond_with_internal_error(err.to_string());
    }

    responder.respond(SetSessionModeResponse::new())
}

pub async fn close_session(
    req: CloseSessionRequest,
    responder: Responder<CloseSessionResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    let session_id = req.session_id.to_string();
    info!("Received close session request for {session_id}");

    responder.respond(CloseSessionResponse::new())
}

pub async fn set_session_config_option(
    req: SetSessionConfigOptionRequest,
    responder: Responder<SetSessionConfigOptionResponse>,
    _: ConnectionTo<Client>,
    registry: SharedRegistry,
    setting: Setting,
) -> Result<(), Error> {
    info!("Received set session config option request {req:?}");

    let session_id = req.session_id.to_string();
    let config_id = req.config_id.to_string();
    let value = req.value.to_string();

    let (mut session, setting) = {
        let reg = registry.lock().await;
        match reg.get_session(&session_id) {
            Some(session) => (session, setting),
            None => {
                return responder.respond_with_error(Error::resource_not_found(Some(session_id)));
            },
        }
    };

    let current_mode = match config_id.as_str() {
        MODE_CONFIG_ID => {
            let agent_type = AgentType::from_id(&value);
            if let Err(err) = session.switch_agent(agent_type).await {
                return responder.respond_with_internal_error(err.to_string());
            }
            agent_type
        },
        MODEL_CONFIG_ID => {
            // TODO: switch the active model on the session once supported.
            AgentType::Build
        },
        _ => AgentType::Build,
    };

    responder.respond(SetSessionConfigOptionResponse::new(config_options(&setting, current_mode)))
}

pub async fn resume_session(
    req: ResumeSessionRequest,
    responder: Responder<ResumeSessionResponse>,
    _: ConnectionTo<Client>,
) -> Result<(), Error> {
    info!("Received resume session request {req:?}");

    responder.respond(ResumeSessionResponse::new())
}

const MODE_CONFIG_ID: &str = "mode";
const MODEL_CONFIG_ID: &str = "model";

fn config_options(setting: &Setting, current_mode: AgentType) -> Vec<SessionConfigOption> {
    vec![mode_option(current_mode), model_option(setting)]
}

fn mode_option(current: AgentType) -> SessionConfigOption {
    let options = [AgentType::Build, AgentType::Plan, AgentType::General, AgentType::Explore]
        .into_iter()
        .map(|agent| {
            SessionConfigSelectOption::new(agent.id(), agent.name())
                .description(Some(agent.description().to_owned()))
        })
        .collect::<Vec<_>>();

    SessionConfigOption::select(
        MODE_CONFIG_ID,
        "Mode",
        current.id(),
        SessionConfigSelectOptions::Ungrouped(options),
    )
    .category(SessionConfigOptionCategory::Mode)
}

fn model_option(setting: &Setting) -> SessionConfigOption {
    let options = setting
        .models
        .iter()
        .map(|m| {
            let alias: String = m.alias().into();
            SessionConfigSelectOption::new(alias.clone(), alias)
        })
        .collect::<Vec<_>>();

    SessionConfigOption::select(
        MODEL_CONFIG_ID,
        "Model",
        setting.active.clone(),
        SessionConfigSelectOptions::Ungrouped(options),
    )
    .category(SessionConfigOptionCategory::Model)
}
