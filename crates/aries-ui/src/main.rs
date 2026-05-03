// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use tauri::{LogicalSize, Manager};
use tokio::sync::Mutex;

mod chat;
mod projects;
mod state;
mod types;

use chat::{bootstrap_chat, get_system_prompt, send_chat_message};
use projects::{list_projects, open_project};
use state::{AppState, SharedState};

#[tauri::command]
fn resize_window_for_chat(window: tauri::Window) -> Result<(), String> {
    let monitor = window.current_monitor().map_err(|e| e.to_string())?
        .ok_or_else(|| "no monitor".to_string())?;
    let logical_size = monitor.size().to_logical::<f64>(monitor.scale_factor());
    let width = (logical_size.width * 0.75).round();
    let height = (logical_size.height * 0.75).round();
    window.set_size(LogicalSize::new(width, height)).map_err(|e| e.to_string())?;
    window.center().map_err(|e| e.to_string())?;
    Ok(())
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
        .manage(Arc::new(Mutex::new(None::<AppState>)) as SharedState)
        .invoke_handler(tauri::generate_handler![
            bootstrap_chat,
            send_chat_message,
            get_system_prompt,
            list_projects,
            open_project,
            resize_window_for_chat
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
