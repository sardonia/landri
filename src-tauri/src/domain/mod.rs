use serde::{Deserialize, Serialize};

pub type ModelKey = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfModelSummary {
    pub repo_id: String,
    pub description: Option<String>,
    pub likes: Option<u64>,
    pub downloads: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HfFile {
    pub rfilename: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModelState {
    NotInstalled,
    Downloading,
    Verifying,
    Installed,
    Corrupt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModel {
    pub model_key: ModelKey,
    pub repo_id: String,
    pub filename: String,
    pub local_path: String,
    pub size_bytes: u64,
    pub verified: bool,
    pub installed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GenState {
    Idle,
    Preparing,
    Generating,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingParams {
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: u32,
    pub repeat_penalty: f32,
    pub max_tokens: u32,
    pub stop_sequences: Vec<String>,
}

impl Default for SamplingParams {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.95,
            top_k: 40,
            repeat_penalty: 1.1,
            max_tokens: 512,
            stop_sequences: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub model_key: ModelKey,
    pub prompt: String,
    pub params: SamplingParams,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadRequest {
    pub repo_id: String,
    pub filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobStarted {
    pub job_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub hf_token: Option<String>,
    pub default_repo_id: String,
    pub default_quant_hint: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hf_token: None,
            // User preference in this conversation: bartowski + phi-3 gguf
            default_repo_id: "bartowski/Phi-3-mini-4k-instruct-v0.3-GGUF".to_string(),
            // User preference: Q5 (default to option 2, Q5)
            default_quant_hint: "Q5".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItemSummary {
    pub id: String,
    pub created_at: i64,
    pub model_key: ModelKey,
    pub repo_id: String,
    pub filename: String,
    pub prompt_preview: String,
    pub output_preview: String,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryItemDetail {
    pub id: String,
    pub created_at: i64,
    pub model_key: ModelKey,
    pub repo_id: String,
    pub filename: String,
    pub prompt: String,
    pub output: String,
    pub params: SamplingParams,
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportRequest {
    pub filename_hint: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub path: String,
}
