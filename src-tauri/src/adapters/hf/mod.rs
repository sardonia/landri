use std::path::Path;

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::Deserialize;
use tokio::{fs::File, io::AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{HfFile, HfModelSummary},
    error::{AppError, AppResult},
    ports::hf::HuggingFacePort,
};

#[derive(Clone)]
pub struct HuggingFaceClient {
    client: reqwest::Client,
}

impl HuggingFaceClient {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Landry/0.1")
            .build()
            .expect("reqwest client");
        Self { client }
    }
}

#[derive(Debug, Deserialize)]
struct HfSearchItem {
    #[serde(alias = "id", alias = "modelId")]
    pub repo_id: String,

    pub likes: Option<u64>,
    pub downloads: Option<u64>,

    #[serde(default)]
    pub cardData: Option<HfCardData>,
}

#[derive(Debug, Deserialize)]
struct HfCardData {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HfModelDetail {
    #[serde(default)]
    pub siblings: Vec<HfSibling>,
}

#[derive(Debug, Deserialize)]
struct HfSibling {
    pub rfilename: String,
    #[serde(default)]
    pub lfs: Option<HfLfs>,
}

#[derive(Debug, Deserialize)]
struct HfLfs {
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub oid: Option<String>,
}

#[async_trait]
impl HuggingFacePort for HuggingFaceClient {
    async fn search_models(&self, query: &str, limit: usize) -> AppResult<Vec<HfModelSummary>> {
        let url = format!(
            "https://huggingface.co/api/models?search={}&limit={}",
            urlencoding::encode(query),
            limit
        );

        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AppError::Network(format!(
                "Hugging Face search failed: HTTP {}",
                resp.status()
            )));
        }

        let items: Vec<HfSearchItem> = resp
            .json()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        Ok(items
            .into_iter()
            .map(|it| HfModelSummary {
                repo_id: it.repo_id,
                description: it
                    .cardData
                    .and_then(|c| c.summary.or(c.description))
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
                likes: it.likes,
                downloads: it.downloads,
            })
            .collect())
    }

    async fn list_model_files(&self, repo_id: &str) -> AppResult<Vec<HfFile>> {
        let url = format!("https://huggingface.co/api/models/{}", repo_id);
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AppError::Network(format!(
                "Hugging Face model detail failed: HTTP {}",
                resp.status()
            )));
        }

        let detail: HfModelDetail = resp
            .json()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        let mut files: Vec<HfFile> = detail
            .siblings
            .into_iter()
            .filter(|s| s.rfilename.to_lowercase().ends_with(".gguf"))
            .map(|s| HfFile {
                rfilename: s.rfilename,
                size: s.lfs.as_ref().and_then(|l| l.size),
                sha256: s.lfs.as_ref().and_then(|l| l.oid.clone()),
            })
            .collect();

        // Sort small->large then name (so Q4/Q5 often float up; not perfect but ok)
        files.sort_by(|a, b| a.size.cmp(&b.size).then(a.rfilename.cmp(&b.rfilename)));

        Ok(files)
    }

    async fn download_file(
        &self,
        repo_id: &str,
        filename: &str,
        dest_path: &Path,
        token: Option<&str>,
        cancel: CancellationToken,
        on_progress: Box<dyn Fn(u64, Option<u64>) + Send + Sync>,
    ) -> AppResult<u64> {
        let url = format!(
            "https://huggingface.co/{}/resolve/main/{}",
            repo_id,
            urlencoding::encode(filename)
        );

        let mut headers = HeaderMap::new();
        if let Some(t) = token {
            if !t.trim().is_empty() {
                let v = HeaderValue::from_str(&format!("Bearer {}", t.trim()))
                    .map_err(|e| AppError::Network(e.to_string()))?;
                headers.insert(AUTHORIZATION, v);
            }
        }

        let resp = self
            .client
            .get(url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(AppError::Network(format!(
                "Download failed: HTTP {}",
                resp.status()
            )));
        }

        let total = resp.content_length();
        let mut stream = resp.bytes_stream();

        let mut file = File::create(dest_path)
            .await
            .map_err(|e| AppError::Fs(e.to_string()))?;

        let mut downloaded: u64 = 0;
        on_progress(downloaded, total);

        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    let _ = file.shutdown().await;
                    let _ = tokio::fs::remove_file(dest_path).await;
                    return Err(AppError::Cancelled);
                }
                next = stream.next() => {
                    match next {
                        Some(Ok(chunk)) => {
                            file.write_all(&chunk)
                                .await
                                .map_err(|e| AppError::Fs(e.to_string()))?;
                            downloaded += chunk.len() as u64;
                            on_progress(downloaded, total);
                        }
                        Some(Err(e)) => {
                            let _ = file.shutdown().await;
                            let _ = tokio::fs::remove_file(dest_path).await;
                            return Err(AppError::Network(e.to_string()));
                        }
                        None => break,
                    }
                }
            }
        }

        file.flush().await.map_err(|e| AppError::Fs(e.to_string()))?;
        Ok(downloaded)
    }
}
