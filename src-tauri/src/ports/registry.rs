use async_trait::async_trait;

use crate::{domain::InstalledModel, error::AppResult};

#[async_trait]
pub trait ModelRegistryPort: Send + Sync {
    async fn list_installed(&self) -> AppResult<Vec<InstalledModel>>;

    async fn get_installed(&self, model_key: &str) -> AppResult<Option<InstalledModel>>;

    async fn upsert_installed(&self, model: &InstalledModel) -> AppResult<()>;

    async fn delete_installed(&self, model_key: &str) -> AppResult<()>;
}
