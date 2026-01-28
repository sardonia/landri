use async_trait::async_trait;

use crate::{
    domain::{HistoryItemDetail, HistoryItemSummary},
    error::AppResult,
};

#[async_trait]
pub trait HistoryPort: Send + Sync {
    async fn list_history(&self, limit: usize) -> AppResult<Vec<HistoryItemSummary>>;

    async fn get_history(&self, id: &str) -> AppResult<Option<HistoryItemDetail>>;

    async fn insert_history(&self, item: &HistoryItemDetail) -> AppResult<()>;

    async fn clear_history(&self) -> AppResult<()>;
}
