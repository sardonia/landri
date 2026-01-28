use async_trait::async_trait;

use crate::{domain::Settings, error::AppResult};

#[async_trait]
pub trait SettingsPort: Send + Sync {
    async fn load_settings(&self) -> AppResult<Settings>;

    async fn save_settings(&self, settings: &Settings) -> AppResult<()>;
}
