use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use tokio::{fs, sync::mpsc};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{InstalledModel, ModelState},
    error::{AppError, AppResult},
    events::{DownloadDoneEvent, DownloadProgressEvent, ToastEvent, EVENT_DOWNLOAD_DONE, EVENT_DOWNLOAD_PROGRESS, EVENT_TOAST_ERROR, EVENT_TOAST_INFO},
    managers::job_manager::JobManager,
    ports::{events::{EventsPort, emit_ser}, hf::HuggingFacePort, registry::ModelRegistryPort},
};

/// Actor-ish manager responsible for downloads + verification + registry updates.
#[derive(Clone)]
pub struct ModelManager {
    pub job_manager: JobManager,
    hf: Arc<dyn HuggingFacePort>,
    registry: Arc<dyn ModelRegistryPort>,
    events: Arc<dyn EventsPort>,
    models_dir: PathBuf,
}

impl ModelManager {
    pub fn new(
        job_manager: JobManager,
        hf: Arc<dyn HuggingFacePort>,
        registry: Arc<dyn ModelRegistryPort>,
        events: Arc<dyn EventsPort>,
        models_dir: PathBuf,
    ) -> Self {
        Self {
            job_manager,
            hf,
            registry,
            events,
            models_dir,
        }
    }

pub async fn start_download(&self, repo_id: String, filename: String, hf_token: Option<String>) -> AppResult<String> {
    let (job_id, cancel) = self.job_manager.create_job().await;
    let job_id_ret = job_id.clone();

    let hf = self.hf.clone();
    let registry = self.registry.clone();
    let events = self.events.clone();
    let models_dir = self.models_dir.clone();
    let jm = self.job_manager.clone();

    tokio::spawn(async move {
        let events_for_flow = events.clone();
        let res = download_flow(
            &job_id,
            &repo_id,
            &filename,
            hf_token.as_deref(),
            cancel.clone(),
            hf,
            registry,
            events_for_flow,
            models_dir,
        )
        .await;

        if let Err(err) = res {
            let toast = match err {
                AppError::Cancelled => ToastEvent {
                    title: "Download cancelled".to_string(),
                    message: "The download was cancelled.".to_string(),
                    detail: Some(format!("{repo_id} / {filename}")),
                    remediation: Some("You can restart the download any time.".to_string()),
                    kind: "info".to_string(),
                },
                _ => ToastEvent {
                    title: "Download failed".to_string(),
                    message: "Unable to download the model.".to_string(),
                    detail: Some(err.to_string()),
                    remediation: Some(
                        "Check your internet connection and (if needed) your Hugging Face token in Settings.".to_string(),
                    ),
                    kind: "error".to_string(),
                },
            };
            let _ = emit_ser(events.as_ref(), EVENT_TOAST_ERROR, &toast);
        }

        jm.remove(&job_id).await;
    });

    Ok(job_id_ret)
}

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }
}

fn sanitize_repo_dir(repo_id: &str) -> String {
    repo_id.replace('/', "--")
}

fn model_key(repo_id: &str, filename: &str) -> String {
    format!("{repo_id}::{filename}")
}

async fn download_flow(
    job_id: &str,
    repo_id: &str,
    filename: &str,
    hf_token: Option<&str>,
    cancel: CancellationToken,
    hf: Arc<dyn HuggingFacePort>,
    registry: Arc<dyn ModelRegistryPort>,
    events: Arc<dyn EventsPort>,
    models_dir: PathBuf,
) -> AppResult<()> {
    let repo_dir = models_dir.join(sanitize_repo_dir(repo_id));
    fs::create_dir_all(&repo_dir).await?;

    let final_path = repo_dir.join(filename);
    let temp_path = repo_dir.join(format!(".{filename}.part"));

    // (Re)start clean.
    if fs::try_exists(&temp_path).await.unwrap_or(false) {
        let _ = fs::remove_file(&temp_path).await;
    }

    // Bounded progress channel for backpressure.
    let (prog_tx, mut prog_rx) = mpsc::channel::<(u64, Option<u64>)>(64);

    let prog_tx_dl = prog_tx.clone();

    // Reporter task that throttles UI events.
    let events_reporter = events.clone();
    let job_id_r = job_id.to_string();
    let repo_id_r = repo_id.to_string();
    let filename_r = filename.to_string();
    let cancel_r = cancel.clone();

    let reporter = tokio::spawn(async move {
        let mut last_emit = Instant::now();
        let mut latest: Option<(u64, Option<u64>)> = None;

        loop {
            tokio::select! {
                _ = cancel_r.cancelled() => {
                    break;
                }
                msg = prog_rx.recv() => {
                    match msg {
                        Some((dl, total)) => {
                            latest = Some((dl, total));
                            let elapsed = last_emit.elapsed();
                            if elapsed.as_millis() >= 150 {
                                if let Some((dl, total)) = latest.take() {
                                    let percent = total.and_then(|t| if t > 0 { Some((dl as f64 / t as f64) * 100.0) } else { None });
                                    let evt = DownloadProgressEvent {
                                        job_id: job_id_r.clone(),
                                        repo_id: repo_id_r.clone(),
                                        filename: filename_r.clone(),
                                        bytes_downloaded: dl,
                                        total_bytes: total,
                                        percent,
                                        state: ModelState::Downloading,
                                    };
                                    let _ = emit_ser(events_reporter.as_ref(), EVENT_DOWNLOAD_PROGRESS, &evt);
                                    last_emit = Instant::now();
                                }
                            }
                        }
                        None => {
                            // Final flush
                            if let Some((dl, total)) = latest.take() {
                                let percent = total.and_then(|t| if t > 0 { Some((dl as f64 / t as f64) * 100.0) } else { None });
                                let evt = DownloadProgressEvent {
                                    job_id: job_id_r.clone(),
                                    repo_id: repo_id_r.clone(),
                                    filename: filename_r.clone(),
                                    bytes_downloaded: dl,
                                    total_bytes: total,
                                    percent,
                                    state: ModelState::Downloading,
                                };
                                let _ = emit_ser(events_reporter.as_ref(), EVENT_DOWNLOAD_PROGRESS, &evt);
                            }
                            break;
                        }
                    }
                }
            }
        }
    });

    // The actual download.
    let written_bytes = hf
        .download_file(
            repo_id,
            filename,
            &temp_path,
            hf_token,
            cancel.clone(),
            Box::new(move |dl, total| {
                // If the receiver is full, drop updates (backpressure).
                let _ = prog_tx_dl.try_send((dl, total));
            }),
        )
        .await?;

    // Verify step (MVP): file exists and size > 0.
    if cancel.is_cancelled() {
        let _ = fs::remove_file(&temp_path).await;
        return Err(AppError::Cancelled);
    }

    let verify_evt = DownloadProgressEvent {
        job_id: job_id.to_string(),
        repo_id: repo_id.to_string(),
        filename: filename.to_string(),
        bytes_downloaded: written_bytes,
        total_bytes: Some(written_bytes),
        percent: Some(100.0),
        state: ModelState::Verifying,
    };
    let _ = emit_ser(events.as_ref(), EVENT_DOWNLOAD_PROGRESS, &verify_evt);

    if written_bytes == 0 {
        let _ = fs::remove_file(&temp_path).await;
        return Err(AppError::Fs("Downloaded file is empty".to_string()));
    }

    // Atomic-ish finalize.
    if fs::try_exists(&final_path).await.unwrap_or(false) {
        let _ = fs::remove_file(&final_path).await;
    }
    fs::rename(&temp_path, &final_path).await?;

    let now = chrono::Utc::now().timestamp();
    let installed = InstalledModel {
        model_key: model_key(repo_id, filename),
        repo_id: repo_id.to_string(),
        filename: filename.to_string(),
        local_path: final_path.to_string_lossy().to_string(),
        size_bytes: written_bytes,
        verified: true,
        installed_at: now,
    };

    registry.upsert_installed(&installed).await?;

    // Let the reporter task end.
    drop(prog_tx);
    let _ = reporter.await;

    let done = DownloadDoneEvent {
        job_id: job_id.to_string(),
        model_key: installed.model_key.clone(),
        repo_id: installed.repo_id.clone(),
        filename: installed.filename.clone(),
        local_path: installed.local_path.clone(),
        size_bytes: installed.size_bytes,
        verified: installed.verified,
    };
    emit_ser(events.as_ref(), EVENT_DOWNLOAD_DONE, &done)?;

    let toast = ToastEvent {
        title: "Model installed".to_string(),
        message: "The model is ready to use.".to_string(),
        detail: Some(format!("{} ({})", installed.repo_id, installed.filename)),
        remediation: Some("Select it as your Active Model and try a prompt.".to_string()),
        kind: "info".to_string(),
    };
    let _ = emit_ser(events.as_ref(), EVENT_TOAST_INFO, &toast);

    Ok(())
}
