use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use uuid::Uuid;

use crate::{
    domain::{GenState, GenerateRequest, HistoryItemDetail},
    error::{AppError, AppResult},
    events::{
        GenDoneEvent, GenTokenEvent, ToastEvent, EVENT_GEN_DONE, EVENT_GEN_TOKEN, EVENT_TOAST_ERROR,
        EVENT_TOAST_INFO,
    },
    managers::job_manager::JobManager,
    ports::{
        events::emit_ser,
        events::EventsPort,
        history::HistoryPort,
        inference::InferencePort,
        registry::ModelRegistryPort,
    },
};

#[derive(Clone)]
pub struct GenerationOrchestrator {
    registry: Arc<dyn ModelRegistryPort>,
    inference: Arc<dyn InferencePort>,
    history: Arc<dyn HistoryPort>,
    events: Arc<dyn EventsPort>,
    jobs: JobManager,
}

impl GenerationOrchestrator {
    pub fn new(
        registry: Arc<dyn ModelRegistryPort>,
        inference: Arc<dyn InferencePort>,
        history: Arc<dyn HistoryPort>,
        events: Arc<dyn EventsPort>,
        jobs: JobManager,
    ) -> Self {
        Self {
            registry,
            inference,
            history,
            events,
            jobs,
        }
    }

    pub async fn start_generation(&self, req: GenerateRequest, hf_token: Option<&str>) -> AppResult<String> {
        if req.prompt.trim().is_empty() {
            return Err(AppError::InvalidInput("Please enter a prompt.".to_string()));
        }

        let installed = self
            .registry
            .get_installed(&req.model_key)
            .await?
            .ok_or_else(|| AppError::InvalidInput("Selected model is not installed.".to_string()))?;

        let (job_id, cancel) = self.jobs.create_job().await;

        
        let job_id_ret = job_id.clone();
        let hf_token_owned: Option<String> = hf_token.map(|s| s.to_string());
let events = self.events.clone();
        let history = self.history.clone();
        let inference = self.inference.clone();
        let jobs = self.jobs.clone();

        let params = req.params.clone();
        let prompt = req.prompt.clone();
        let model_key = req.model_key.clone();

        tokio::spawn(async move {
            let started_at = Instant::now();

            let _ = emit_ser(
                events.as_ref(),
                EVENT_TOAST_INFO,
                &ToastEvent {
                    title: "Generating".to_string(),
                    message: "Preparing model and starting generation…".to_string(),
                    detail: None,
                    remediation: None,
                    kind: "info".to_string(),
                },
            );

            let output_buf: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
            let total_chars: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));

            // Ensure model loaded.
            if let Err(e) = inference.ensure_model_loaded(&installed, hf_token_owned.as_deref()).await {
                let _ = emit_ser(
                    events.as_ref(),
                    EVENT_TOAST_ERROR,
                    &ToastEvent {
                        title: "Model load failed".to_string(),
                        message: e.to_string(),
                        detail: Some(format!("{} / {}", installed.repo_id, installed.filename)),
                        remediation: Some(
                            "Make sure the GGUF file is valid, and set a Hugging Face token in Settings if the repo is gated.".to_string(),
                        ),
                        kind: "error".to_string(),
                    },
                );
                jobs.remove(&job_id).await;
                return;
            }

            let events2 = events.clone();
            let output_buf2 = output_buf.clone();
            let total_chars2 = total_chars.clone();
            let job_id2 = job_id.clone();
            let started_at2 = started_at;

            let on_text = Box::new(move |chunk: String| {
                // Accumulate output.
                {
                    if let Ok(mut g) = output_buf2.lock() {
                        g.push_str(&chunk);
                    }
                    if let Ok(mut t) = total_chars2.lock() {
                        *t += chunk.chars().count() as u64;
                    }
                }

                let total_now = total_chars2.lock().map(|t| *t).unwrap_or(0);
                let _ = emit_ser(
                    events2.as_ref(),
                    EVENT_GEN_TOKEN,
                    &GenTokenEvent {
                        job_id: job_id2.clone(),
                        state: GenState::Generating,
                        chunk,
                        total_chars: total_now,
                        elapsed_ms: started_at2.elapsed().as_millis() as u64,
                    },
                );
            });

            let usage_res = inference
                .stream_generate(&installed, &prompt, &params, cancel.clone(), on_text)
                .await;

            match usage_res {
                Ok(usage) => {
                    let created_at = chrono::Utc::now().timestamp();
                    let session_id = Uuid::new_v4().to_string();

                    let output = output_buf.lock().map(|g| g.clone()).unwrap_or_default();
                    let params_json = serde_json::to_string(&params).unwrap_or_else(|_| "{}".to_string());

                    let item = HistoryItemDetail {
                        id: session_id.clone(),
                        created_at,
                        model_key: model_key.clone(),
                        repo_id: installed.repo_id.clone(),
                        filename: installed.filename.clone(),
                        prompt: prompt.clone(),
                        output: output.clone(),
                        params_json,
                        prompt_tokens: usage.prompt_tokens,
                        completion_tokens: usage.completion_tokens,
                        total_tokens: usage.total_tokens,
                    };

                    let _ = history.insert_history(&item).await;

                    let total_now = total_chars.lock().map(|t| *t).unwrap_or(0);
                    let _ = emit_ser(
                        events.as_ref(),
                        EVENT_GEN_DONE,
                        &GenDoneEvent {
                            job_id: job_id.clone(),
                            state: GenState::Done,
                            session_id: Some(session_id),
                            total_chars: total_now,
                            prompt_tokens: usage.prompt_tokens,
                            completion_tokens: usage.completion_tokens,
                            total_tokens: usage.total_tokens,
                        },
                    );
                }
                Err(AppError::Cancelled) => {
                    let total_now = total_chars.lock().map(|t| *t).unwrap_or(0);
                    let _ = emit_ser(
                        events.as_ref(),
                        EVENT_GEN_DONE,
                        &GenDoneEvent {
                            job_id: job_id.clone(),
                            state: GenState::Cancelled,
                            session_id: None,
                            total_chars: total_now,
                            prompt_tokens: None,
                            completion_tokens: None,
                            total_tokens: None,
                        },
                    );
                }
                Err(e) => {
                    let _ = emit_ser(
                        events.as_ref(),
                        EVENT_TOAST_ERROR,
                        &ToastEvent {
                            title: "Generation failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            remediation: Some(
                                "Try a smaller quantization, lower max tokens, or ensure your system has enough RAM.".to_string(),
                            ),
                            kind: "error".to_string(),
                        },
                    );

                    let total_now = total_chars.lock().map(|t| *t).unwrap_or(0);
                    let _ = emit_ser(
                        events.as_ref(),
                        EVENT_GEN_DONE,
                        &GenDoneEvent {
                            job_id: job_id.clone(),
                            state: GenState::Failed,
                            session_id: None,
                            total_chars: total_now,
                            prompt_tokens: None,
                            completion_tokens: None,
                            total_tokens: None,
                        },
                    );
                }
            }

            jobs.remove(&job_id).await;
        });

        Ok(job_id_ret)
    }
}
