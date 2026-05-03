// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use tauri::{LogicalSize, Manager};
use tokio::sync::Mutex;

mod chat;
mod state;
mod types;

use chat::{bootstrap_chat, send_chat_message};
use state::{AppState, SharedState};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                if let Some(monitor) = window.current_monitor()? {
                    let logical_size = monitor.size().to_logical::<f64>(monitor.scale_factor());
                    let width = (logical_size.width * 0.75).round();
                    let height = (logical_size.height * 0.75).round();
                    window.set_size(LogicalSize::new(width, height))?;
                    window.center()?;
                }
            }
            Ok(())
        })
        .manage(Arc::new(Mutex::new(None::<AppState>)) as SharedState)
        .invoke_handler(tauri::generate_handler![bootstrap_chat, send_chat_message])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
