use std::path::Path;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{HfFile, HfModelSummary},
    error::AppResult,
};

#[async_trait]
pub trait HuggingFacePort: Send + Sync {
    async fn search_models(&self, query: &str, limit: usize) -> AppResult<Vec<HfModelSummary>>;

    async fn list_model_files(&self, repo_id: &str) -> AppResult<Vec<HfFile>>;

    /// Download a file to `dest_path`.
    ///
    /// Returns the final size (bytes) written.
    async fn download_file(
        &self,
        repo_id: &str,
        filename: &str,
        dest_path: &Path,
        hf_token: Option<&str>,
        cancel: CancellationToken,
        on_progress: Box<dyn Fn(u64, Option<u64>) + Send + Sync>,
    ) -> AppResult<u64>;
}
