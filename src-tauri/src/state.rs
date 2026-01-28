use std::{path::PathBuf, sync::Arc};


use tauri::Manager;
use tokio::sync::RwLock;

use crate::{
    adapters::{fs::JsonSettingsStore, hf::HuggingFaceClient, sqlite::SqliteStore, tauri_events::TauriEventEmitter},
    app::{generation_orchestrator::GenerationOrchestrator, model_orchestrator::ModelOrchestrator},
    domain::Settings,
    error::{AppError, AppResult},
    managers::{inference_manager::InferenceManager, job_manager::JobManager, model_manager::ModelManager},
    ports::{history::HistoryPort, registry::ModelRegistryPort, settings::SettingsPort},
};

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub app_data_dir: PathBuf,
    pub models_dir: PathBuf,
    pub exports_dir: PathBuf,
    pub db_path: PathBuf,
    pub settings_path: PathBuf,
}

pub struct AppState {
    pub paths: AppPaths,

    pub settings: RwLock<Settings>,
    settings_store: Arc<dyn SettingsPort>,

    pub model_orchestrator: ModelOrchestrator,
    pub generation_orchestrator: GenerationOrchestrator,

    // Repos for direct read commands.
    pub registry: Arc<dyn ModelRegistryPort>,
    pub history: Arc<dyn HistoryPort>,
    pub jobs: JobManager,
}

impl AppState {
    pub async fn new(app: &tauri::AppHandle) -> AppResult<Self> {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| AppError::Fs(e.to_string()))?;

        let models_dir = app_data_dir.join("models");
        let exports_dir = app_data_dir.join("exports");
        let db_path = app_data_dir.join("landry.sqlite");
        let settings_path = app_data_dir.join("settings.json");

        std::fs::create_dir_all(&models_dir).map_err(|e| AppError::Fs(e.to_string()))?;
        std::fs::create_dir_all(&exports_dir).map_err(|e| AppError::Fs(e.to_string()))?;

        let paths = AppPaths {
            app_data_dir,
            models_dir,
            exports_dir,
            db_path: db_path.clone(),
            settings_path: settings_path.clone(),
        };

        let hf = Arc::new(HuggingFaceClient::new());
        let registry_and_history = Arc::new(SqliteStore::new(db_path)?);

        let settings_store: Arc<dyn SettingsPort> = Arc::new(JsonSettingsStore::new(settings_path));
        let loaded_settings = settings_store.load_settings().await.unwrap_or_default();

        let events: Arc<dyn crate::ports::events::EventsPort> = Arc::new(TauriEventEmitter::new(app.clone()));

        let jobs = JobManager::new();

        let model_manager = Arc::new(ModelManager::new(
            hf.clone(),
            registry_and_history.clone(),
            events.clone(),
            jobs.clone(),
            paths.models_dir.clone(),
        ));

        let model_orchestrator = ModelOrchestrator::new(
            hf.clone(),
            registry_and_history.clone(),
            model_manager,
        );

        let inference: Arc<dyn crate::ports::inference::InferencePort> = Arc::new(InferenceManager::new());

        let generation_orchestrator = GenerationOrchestrator::new(
            registry_and_history.clone(),
            inference,
            registry_and_history.clone(),
            events,
            jobs.clone(),
        );

        Ok(Self {
            paths,
            settings: RwLock::new(loaded_settings),
            settings_store,
            model_orchestrator,
            generation_orchestrator,
            registry: registry_and_history.clone(),
            history: registry_and_history,
            jobs,
        })
    }

    pub async fn get_settings(&self) -> Settings {
        self.settings.read().await.clone()
    }

    pub async fn update_settings(&self, new_settings: Settings) -> AppResult<()> {
        self.settings_store.save_settings(&new_settings).await?;
        *self.settings.write().await = new_settings;
        Ok(())
    }
}
