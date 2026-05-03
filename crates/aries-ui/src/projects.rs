use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const RECENT_PROJECTS_FILE: &str = "recent_projects.json";
const MAX_RECENT_PROJECTS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
}

pub fn recent_projects_path() -> PathBuf {
    let home = std::env::home_dir().unwrap_or_default();
    home.join(".local").join("share").join("aries").join(RECENT_PROJECTS_FILE)
}

pub fn load_recent_projects() -> Vec<ProjectEntry> {
    let path = recent_projects_path();
    if !path.exists() {
        return vec![];
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return vec![];
    };
    serde_json::from_str(&content).unwrap_or_default()
}

pub fn save_recent_projects(projects: &[ProjectEntry]) {
    let path = recent_projects_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let content = serde_json::to_string_pretty(projects).unwrap_or_default();
    let _ = std::fs::write(&path, content);
}

pub fn add_to_recent(project_path: &str) {
    let mut projects = load_recent_projects();

    // Remove existing entry with same path
    projects.retain(|p| p.path != project_path);

    let name = Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| project_path.to_string());

    let branch = detect_git_branch(project_path);

    projects.insert(0, ProjectEntry { name, path: project_path.to_string(), branch });

    projects.truncate(MAX_RECENT_PROJECTS);
    save_recent_projects(&projects);
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
pub fn list_projects() -> Vec<ProjectEntry> {
    load_recent_projects()
}

#[tauri::command]
pub async fn open_project(path: String) -> Result<(), String> {
    let p = Path::new(&path);
    if !p.exists() || !p.is_dir() {
        return Err(format!("Directory does not exist: {}", path));
    }
    add_to_recent(&path);
    Ok(())
}
