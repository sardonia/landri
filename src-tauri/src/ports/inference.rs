use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{InstalledModel, SamplingParams},
    error::AppResult,
};

#[derive(Debug, Clone)]
pub struct GenUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[async_trait]
pub trait InferencePort: Send + Sync {
    async fn ensure_model_loaded(&self, model: &InstalledModel, hf_token: Option<&str>) -> AppResult<()>;

    /// Stream-generate text for a single-user prompt.
    ///
    /// `on_text` will be called with incremental chunks (already chunked for UI friendliness).
    async fn stream_generate(
        &self,
        model: &InstalledModel,
        prompt: &str,
        params: &SamplingParams,
        cancel: CancellationToken,
        on_text: Box<dyn Fn(String) + Send + Sync>,
    ) -> AppResult<GenUsage>;
}
