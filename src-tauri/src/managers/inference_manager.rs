use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use futures_util::StreamExt;
use mistralrs::{GgufModelBuilder, Model, Response, TextMessageRole, TextMessages, TokenSource};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{InstalledModel, SamplingParams},
    error::{AppError, AppResult},
    ports::inference::{GenUsage, InferencePort},
};

#[derive(Default)]
struct Loaded {
    key: Option<String>,
    model: Option<Arc<Model>>,
}

/// Actor-ish manager that owns loaded model(s) and performs streaming generation.
#[derive(Clone, Default)]
pub struct InferenceManager {
    inner: Arc<Mutex<Loaded>>,
}

impl InferenceManager {
    pub fn new() -> Self {
        Self::default()
    }

    async fn build_model(
        local_dir: &Path,
        filename: &str,
        repo_id: &str,
        hf_token: Option<&str>,
    ) -> AppResult<Model> {
        // NOTE: mistralrs `GgufModelBuilder::new` takes a model directory and a list of GGUF filenames
        // relative to that directory.
        let mut builder = GgufModelBuilder::new(local_dir.to_string_lossy().to_string(), vec![filename.to_string()]);

        // Heuristic: many GGUF repos do not ship tokenizer/chat template; point to an upstream HF repo.
        let tok_model_id = if repo_id.to_lowercase().contains("phi-3-mini-4k-instruct") {
            "microsoft/Phi-3-mini-4k-instruct".to_string()
        } else {
            repo_id.to_string()
        };
        builder = builder.with_tok_model_id(tok_model_id);

        builder = match hf_token {
            Some(token) if !token.trim().is_empty() => builder.with_token_source(TokenSource::Literal(token.to_string())),
            _ => builder.with_token_source(TokenSource::CacheToken),
        };

        builder = builder.with_logging();

        let model = builder.build().await.map_err(|e| AppError::Inference(e.to_string()))?;
        Ok(model)
    }

    fn _unused_sampling_notice(_params: &SamplingParams) {
        // For now, we use mistralrs defaults for sampling when streaming chat.
        // We keep SamplingParams in the app API; upgrading to RequestBuilder-based
        // sampling control can be done without changing UI or commands.
    }
}

#[async_trait]
impl InferencePort for InferenceManager {
    async fn ensure_model_loaded(&self, model: &InstalledModel, hf_token: Option<&str>) -> AppResult<()> {
        let mut guard = self.inner.lock().await;
        if guard.key.as_deref() == Some(model.model_key.as_str()) && guard.model.is_some() {
            return Ok(());
        }

        // Drop previous model.
        guard.model = None;
        guard.key = None;

        let local_path = std::path::PathBuf::from(&model.local_path);
        let local_dir = local_path
            .parent()
            .ok_or_else(|| AppError::Inference("Invalid model path".to_string()))?;

        let filename = local_path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| AppError::Inference("Invalid model filename".to_string()))?;

        let loaded = Self::build_model(local_dir, filename, &model.repo_id, hf_token).await?;
        guard.model = Some(Arc::new(loaded));
        guard.key = Some(model.model_key.clone());
        Ok(())
    }

    async fn stream_generate(
        &self,
        _model: &InstalledModel,
        prompt: &str,
        params: &SamplingParams,
        cancel: CancellationToken,
        on_text: Box<dyn Fn(String) + Send + Sync>,
    ) -> AppResult<GenUsage> {
        Self::_unused_sampling_notice(params);

        // Clone model handle out so we don't hold the mutex during generation.
        let m = {
            let guard = self.inner.lock().await;
            guard
                .model
                .as_ref()
                .ok_or_else(|| AppError::Inference("Model not loaded".to_string()))?
                .clone()
        };

        let messages = TextMessages::new().add_message(TextMessageRole::User, prompt);

        let mut stream = m
            .stream_chat_request(messages)
            .await
            .map_err(|e| AppError::Inference(e.to_string()))?;

        let mut buffer = String::new();
        let mut last_emit = Instant::now();

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    if !buffer.is_empty() {
                        on_text(buffer.clone());
                    }
                    return Err(AppError::Cancelled);
                }
                next = stream.next() => {
                    match next {
                        Some(resp) => match resp {
    Response::Chunk(chunk) => {
        if let Some(choice) = chunk.choices.first() {
            if let Some(content) = choice.delta.content.as_ref() {
                buffer.push_str(content);

                let should_flush = buffer.len() >= 120 || last_emit.elapsed() >= Duration::from_millis(50);
                if should_flush {
                    on_text(buffer.clone());
                    buffer.clear();
                    last_emit = Instant::now();
                }
            }
        }
    }
    Response::Done(_done) => {
        if !buffer.is_empty() {
            on_text(buffer.clone());
            buffer.clear();
        }
        break;
    }
    Response::ModelError(e, _extra) => {
        return Err(AppError::Inference(e));
    }
    // Newer mistralrs variants
    Response::InternalError(e) => {
        return Err(AppError::Inference(e.to_string()));
    }
    Response::ValidationError(e) => {
        return Err(AppError::Inference(e.to_string()));
    }
    Response::CompletionModelError(e, _extra) => {
        return Err(AppError::Inference(e));
    }
    Response::CompletionDone(_done) => {
        if !buffer.is_empty() {
            on_text(buffer.clone());
            buffer.clear();
        }
        break;
    }
    Response::CompletionChunk(_chunk) => {
        // Not used for chat streaming; ignore.
    }
    other => {
        // Be resilient to enum growth: treat unexpected variants as a soft error.
        return Err(AppError::Inference(format!("Unsupported inference response variant")));
    }
},
None => break,
                    }
                }
            }
        }

        Ok(GenUsage {
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
        })
    }
}
