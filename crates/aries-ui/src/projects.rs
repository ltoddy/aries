use std::path::Path;

use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use aries_session::SessionRegistry;

use crate::state::{AppState, SharedState};
use crate::types::ProjectEntry;

async fn ensure_registry(guard: &mut Option<AppState>) -> Result<&mut AppState, String> {
    if guard.is_none() {
        let gctx = GlobalContext::new().map_err(|err| err.to_string())?;
        let loader = AriesConfigLoader::new(&gctx.config_dir);
        let config = loader.load_or_setup().await.map_err(|err| err.to_string())?;
        let provider = config.provider().to_string();
        let model = config.model().to_string();

        let registry = SessionRegistry::new(gctx, config).await.map_err(|err| err.to_string())?;

        *guard = Some(AppState {
            registry,
            provider,
            model,
            active_project: None,
            active_session: None,
        });
    }
    Ok(guard.as_mut().expect("registry initialized"))
}

fn detect_git_branch(project_path: &str) -> Option<String> {
    let head_path = Path::new(project_path).join(".git").join("HEAD");
    let content = std::fs::read_to_string(head_path).ok()?;
    let trimmed = content.trim();
    if let Some(branch) = trimmed.strip_prefix("ref: refs/heads/") {
        Some(branch.to_string())
    } else {
        // Detached HEAD, show short hash
        Some(trimmed.chars().take(8).collect())
    }
}

#[tauri::command]
pub async fn list_projects(
    state: tauri::State<'_, SharedState>,
) -> Result<Vec<ProjectEntry>, String> {
    let mut guard = state.lock().await;
    let app_state = ensure_registry(&mut guard).await?;

    let projects = app_state.registry.list_projects().await.map_err(|err| err.to_string())?;

    let entries = projects
        .into_iter()
        .map(|p| {
            let branch = detect_git_branch(&p.dir);
            ProjectEntry { id: p.id, name: p.name, path: p.dir, branch }
        })
        .collect();

    Ok(entries)
}

#[tauri::command]
pub async fn activate_project(
    path: String,
    state: tauri::State<'_, SharedState>,
) -> Result<ProjectEntry, String> {
    let p = Path::new(&path);
    if !p.exists() || !p.is_dir() {
        return Err(format!("Directory does not exist: {}", path));
    }

    let mut guard = state.lock().await;
    let app_state = ensure_registry(&mut guard).await?;

    let project = app_state.registry.active(&path).await.map_err(|err| err.to_string())?;

    let entry = ProjectEntry {
        id: project.id,
        name: project.name.clone(),
        path: project.dir.clone(),
        branch: detect_git_branch(&project.dir),
    };

    app_state.active_project = Some(project);
    app_state.active_session = None;

    Ok(entry)
}
