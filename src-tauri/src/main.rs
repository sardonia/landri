#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod adapters;
mod app;
mod domain;
mod events;
mod error;
mod managers;
mod ports;
mod state;
mod tauri_api;

use state::AppState;
use tauri::Manager;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse().unwrap()))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let handle = app.handle();
            let state = tauri::async_runtime::block_on(AppState::new(&handle))?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            tauri_api::commands::hf_search_models,
            tauri_api::commands::hf_list_gguf_files,
            tauri_api::commands::list_installed_models,
            tauri_api::commands::download_model,
            tauri_api::commands::cancel_job,
            tauri_api::commands::start_generation,
            tauri_api::commands::get_settings,
            tauri_api::commands::set_settings,
            tauri_api::commands::list_history,
            tauri_api::commands::get_history_item,
            tauri_api::commands::export_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
