// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use tauri::{LogicalSize, Manager};
use tokio::sync::Mutex;

mod chat;
mod projects;
mod state;
mod types;

use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use chat::{bootstrap_chat, clear_history, get_system_prompt, list_sessions, send_chat_message};
use projects::{activate_project, list_projects};
use state::SharedState;
use types::ConfigFormData;

#[tauri::command]
fn resize_window_for_chat(window: tauri::Window) -> Result<(), String> {
    let monitor = window
        .current_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no monitor".to_string())?;
    let logical_size = monitor.size().to_logical::<f64>(monitor.scale_factor());
    let width = (logical_size.width * 0.75).round();
    let height = (logical_size.height * 0.75).round();
    window.set_size(LogicalSize::new(width, height)).map_err(|e| e.to_string())?;
    window.center().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn get_config() -> Result<Option<ConfigFormData>, String> {
    let gctx = GlobalContext::new().map_err(|err| err.to_string())?;
    let loader = AriesConfigLoader::new(&gctx.config_dir);
    match loader.load().await {
        Ok(config) => Ok(Some(ConfigFormData::from_config(&config))),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
async fn save_config(config: ConfigFormData) -> Result<(), String> {
    let gctx = GlobalContext::new().map_err(|err| err.to_string())?;
    let loader = AriesConfigLoader::new(&gctx.config_dir);
    let parsed = config.into_config().map_err(|err| err.to_string())?;
    loader.save(&parsed).await.map_err(|err| err.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                window.set_size(LogicalSize::new(600.0, 500.0))?;
                window.center()?;
            }
            Ok(())
        })
        .manage(Arc::new(Mutex::new(None::<state::AppState>)) as SharedState)
        .invoke_handler(tauri::generate_handler![
            bootstrap_chat,
            send_chat_message,
            get_system_prompt,
            clear_history,
            list_projects,
            activate_project,
            list_sessions,
            resize_window_for_chat,
            get_config,
            save_config
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
