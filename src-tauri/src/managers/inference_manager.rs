use std::{
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use futures_util::StreamExt;
use mistralrs::{
    GgufModelBuilder, Model, RequestBuilder, Response, SamplingParams as MsSamplingParams,
    StopTokens, TextMessageRole, TokenSource,
};
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
        let mut builder = GgufModelBuilder::new(local_dir.to_string_lossy(), vec![filename]);

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

        let model = builder
            .build()
            .await
            .map_err(|e| AppError::Inference(e.to_string()))?;

        Ok(model)
    }

    fn to_mistral_sampling(params: &SamplingParams) -> MsSamplingParams {
        MsSamplingParams {
            temperature: Some(params.temperature),
            top_k: Some(params.top_k as usize),
            top_p: Some(params.top_p),
            min_p: None,
            top_n_logprobs: 0,
            frequency_penalty: None,
            presence_penalty: None,
            repetition_penalty: Some(params.repeat_penalty),
            stop_toks: if params.stop_sequences.is_empty() {
                None
            } else {
                Some(StopTokens::Seqs(params.stop_sequences.clone()))
            },
            max_len: Some(params.max_tokens as usize),
            logits_bias: None,
            n_choices: 1,
            dry_params: None,
        }
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
        model: &InstalledModel,
        prompt: &str,
        params: &SamplingParams,
        cancel: CancellationToken,
        on_text: Box<dyn Fn(String) + Send + Sync>,
    ) -> AppResult<GenUsage> {
        // Clone model handle out so we don't hold the mutex during generation.
        let m = {
            let guard = self.inner.lock().await;
            guard
                .model
                .as_ref()
                .ok_or_else(|| AppError::Inference("Model not loaded".to_string()))?
                .clone()
        };

        let request = RequestBuilder::new()
            .add_message(TextMessageRole::User, prompt)
            .set_sampling(Self::to_mistral_sampling(params))
            .build();

        let mut stream = m
            .stream_chat_request(request)
            .await
            .map_err(|e| AppError::Inference(e.to_string()))?;

        let mut buffer = String::new();
        let mut last_emit = Instant::now();

        let mut usage = GenUsage {
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
        };

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

                                        let should_flush = buffer.len() >= 96 || last_emit.elapsed() >= Duration::from_millis(40);
                                        if should_flush {
                                            on_text(buffer.clone());
                                            buffer.clear();
                                            last_emit = Instant::now();
                                        }
                                    }
                                }
                            }
                            Response::Done(done) => {
                                if !buffer.is_empty() {
                                    on_text(buffer.clone());
                                    buffer.clear();
                                }

                                if let Some(u) = done.usage {
                                    usage.prompt_tokens = u.prompt_tokens.map(|v| v as u32);
                                    usage.completion_tokens = u.completion_tokens.map(|v| v as u32);
                                    usage.total_tokens = u.total_tokens.map(|v| v as u32);
                                }
                                break;
                            }
                            Response::ModelError(e) => {
                                return Err(AppError::Inference(e));
                            }
                        },
                        None => break,
                    }
                }
            }
        }

        Ok(usage)
    }
}
