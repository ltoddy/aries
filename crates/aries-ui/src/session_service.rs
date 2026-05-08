use std::path::PathBuf;

use aries_session::{Session, SessionRegistry};

use crate::state::AppState;
use crate::types::SessionSummary;

fn require_project_dir(app_state: &AppState) -> Result<String, String> {
    app_state.active_project_dir.clone().ok_or_else(|| "no active project".to_string())
}

fn require_active_session_id(app_state: &AppState) -> Result<String, String> {
    app_state.active_session_id.clone().ok_or_else(|| "active session not found".to_string())
}

async fn validate_session_project(
    app_state: &mut AppState,
    session_id: &str,
    project_dir: &str,
) -> Result<(), String> {
    let metadata = app_state
        .registry
        .list_sessions(None)
        .await
        .map_err(|err| err.to_string())?
        .into_iter()
        .find(|session| session.session_id == session_id)
        .ok_or_else(|| format!("session not found: {session_id}"))?;

    if metadata.cwd != project_dir {
        return Err(format!(
            "session {session_id} does not belong to active project: expected {project_dir}, got {}",
            metadata.cwd
        ));
    }

    Ok(())
}

pub async fn list_sessions(app_state: &mut AppState) -> Result<Vec<SessionSummary>, String> {
    let project_dir = require_project_dir(app_state)?;
    let sessions = app_state
        .registry
        .list_sessions(Some(PathBuf::from(project_dir)))
        .await
        .map_err(|err| err.to_string())?;

    Ok(sessions
        .into_iter()
        .map(|s| SessionSummary {
            id: s.session_id.clone(),
            session_id: s.session_id,
            title: s.title,
            project_dir: s.cwd,
        })
        .collect())
}

pub async fn load_session(
    app_state: &mut AppState,
    session_id: Option<String>,
) -> Result<Session, String> {
    let project_dir = require_project_dir(app_state)?;

    let session = match session_id {
        Some(session_id) => {
            validate_session_project(app_state, &session_id, &project_dir).await?;
            app_state.registry.load_session(&session_id).await.map_err(|err| err.to_string())?
        },
        None => {
            app_state.registry.new_session(&project_dir).await.map_err(|err| err.to_string())?
        },
    };

    app_state.active_session_id = Some(session.id());
    Ok(session)
}

pub async fn get_session(app_state: &mut AppState) -> Result<Session, String> {
    let session_id = require_active_session_id(app_state)?;
    let project_dir = require_project_dir(app_state)?;

    validate_session_project(app_state, &session_id, &project_dir).await?;

    if let Some(session) = app_state.registry.get_session(&session_id) {
        return Ok(session);
    }

    let session =
        app_state.registry.load_session(&session_id).await.map_err(|err| err.to_string())?;

    Ok(session)
}

pub fn putback_session(registry: &mut SessionRegistry, session: Session) {
    registry.putback_session(session);
}
