use std::path::PathBuf;

use async_trait::async_trait;
use tokio::fs;

use crate::{
    domain::Settings,
    error::{AppError, AppResult},
    ports::settings::SettingsPort,
};

#[derive(Clone)]
pub struct JsonSettingsStore {
    path: PathBuf,
}

impl JsonSettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl SettingsPort for JsonSettingsStore {
    async fn load_settings(&self) -> AppResult<Settings> {
        match fs::read_to_string(&self.path).await {
            Ok(s) => {
                let settings: Settings = serde_json::from_str(&s)
                    .map_err(|e| AppError::Fs(format!("Invalid settings.json: {e}")))?;
                Ok(settings)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(AppError::Fs(e.to_string())),
        }
    }

    async fn save_settings(&self, settings: &Settings) -> AppResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|e| AppError::Fs(e.to_string()))?;
        }

        let json = serde_json::to_string_pretty(settings)
            .map_err(|e| AppError::Fs(e.to_string()))?;
        fs::write(&self.path, json)
            .await
            .map_err(|e| AppError::Fs(e.to_string()))?;
        Ok(())
    }
}
