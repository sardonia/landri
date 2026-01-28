use std::path::PathBuf;

use tauri::State;

use crate::{
    domain::{GenerateRequest, HfFile, HfModelSummary, InstalledModel, Settings},
    error::{AppError, AppResult},
    state::AppState,
};

fn to_err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[tauri::command]
pub async fn hf_search_models(
    state: State<'_, AppState>,
    query: Option<String>,
) -> Result<Vec<HfModelSummary>, String> {
    state
        .model_orchestrator
        .search_models(query)
        .await
        .map_err(to_err)
}

#[tauri::command]
pub async fn hf_list_gguf_files(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<HfFile>, String> {
    state
        .model_orchestrator
        .list_gguf_files(&repo_id)
        .await
        .map_err(to_err)
}

#[tauri::command]
pub async fn list_installed_models(state: State<'_, AppState>) -> Result<Vec<InstalledModel>, String> {
    state.registry.list_installed().await.map_err(to_err)
}

#[tauri::command]
pub async fn download_model(
    state: State<'_, AppState>,
    repo_id: String,
    filename: String,
) -> Result<String, String> {
    let settings = state.get_settings().await;
    state
        .model_orchestrator
        .start_download(&repo_id, &filename, settings.hf_token.as_deref())
        .await
        .map_err(to_err)
}

#[tauri::command]
pub async fn cancel_job(state: State<'_, AppState>, job_id: String) -> Result<bool, String> {
    Ok(state.jobs.cancel(&job_id).await)
}

#[tauri::command]
pub async fn start_generation(state: State<'_, AppState>, req: GenerateRequest) -> Result<String, String> {
    let settings = state.get_settings().await;
    state
        .generation_orchestrator
        .start_generation(req, settings.hf_token.as_deref())
        .await
        .map_err(to_err)
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.get_settings().await)
}

#[tauri::command]
pub async fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<bool, String> {
    state.update_settings(settings).await.map(|_| true).map_err(to_err)
}

#[tauri::command]
pub async fn list_history(state: State<'_, AppState>, limit: Option<u32>) -> Result<Vec<crate::domain::HistoryItemSummary>, String> {
    let limit = limit.unwrap_or(100) as usize;
    state.history.list_history(limit).await.map_err(to_err)
}

#[tauri::command]
pub async fn get_history_item(state: State<'_, AppState>, id: String) -> Result<Option<crate::domain::HistoryItemDetail>, String> {
    state.history.get_history(&id).await.map_err(to_err)
}

#[tauri::command]
pub async fn export_text(
    state: State<'_, AppState>,
    filename: String,
    content: String,
) -> Result<String, String> {
    let safe = sanitize_filename::sanitize(filename);
    let name = if safe.to_lowercase().ends_with(".txt") {
        safe
    } else {
        format!("{safe}.txt")
    };

    let path: PathBuf = state.paths.exports_dir.join(name);
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| to_err(AppError::Fs(e.to_string())))?;

    Ok(path.to_string_lossy().to_string())
}
