use serde::Serialize;

use crate::domain::{GenState, ModelState};

pub const EVENT_DOWNLOAD_PROGRESS: &str = "landry://download/progress";
pub const EVENT_DOWNLOAD_DONE: &str = "landry://download/done";
pub const EVENT_GEN_TOKEN: &str = "landry://gen/token";
pub const EVENT_GEN_DONE: &str = "landry://gen/done";
pub const EVENT_TOAST_ERROR: &str = "landry://toast/error";
pub const EVENT_TOAST_INFO: &str = "landry://toast/info";

#[derive(Debug, Serialize, Clone)]
pub struct DownloadProgressEvent {
    pub job_id: String,
    pub repo_id: String,
    pub filename: String,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub percent: Option<f64>,
    pub state: ModelState,
}

#[derive(Debug, Serialize, Clone)]
pub struct DownloadDoneEvent {
    pub job_id: String,
    pub model_key: String,
    pub repo_id: String,
    pub filename: String,
    pub local_path: String,
    pub size_bytes: u64,
    pub verified: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct GenTokenEvent {
    pub job_id: String,
    pub state: GenState,
    pub chunk: String,
    pub total_chars: u64,
    pub elapsed_ms: u64,
}

#[derive(Debug, Serialize, Clone)]
pub struct GenDoneEvent {
    pub job_id: String,
    pub state: GenState,
    pub session_id: Option<String>,
    pub total_chars: u64,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ToastEvent {
    pub title: String,
    pub message: String,
    pub detail: Option<String>,
    pub remediation: Option<String>,
    pub kind: String,
}
