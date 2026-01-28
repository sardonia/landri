use std::sync::Arc;

use crate::{
    domain::{HfFile, HfModelSummary, InstalledModel},
    error::AppResult,
    managers::model_manager::ModelManager,
    ports::{hf::HuggingFacePort, registry::ModelRegistryPort},
};

#[derive(Clone)]
pub struct ModelOrchestrator {
    hf: Arc<dyn HuggingFacePort>,
    registry: Arc<dyn ModelRegistryPort>,
    model_manager: Arc<ModelManager>,
}

impl ModelOrchestrator {
    pub fn new(
        hf: Arc<dyn HuggingFacePort>,
        registry: Arc<dyn ModelRegistryPort>,
        model_manager: Arc<ModelManager>,
    ) -> Self {
        Self {
            hf,
            registry,
            model_manager,
        }
    }

    pub fn curated_defaults(&self) -> Vec<HfModelSummary> {
        vec![
            HfModelSummary {
                repo_id: "bartowski/Phi-3-mini-4k-instruct-v0.3-GGUF".to_string(),
                description: Some("Phi-3 Mini 4k Instruct (v0.3) - GGUF quantizations (Bartowski)".to_string()),
                likes: None,
                downloads: None,
            },
            HfModelSummary {
                repo_id: "TheBloke/Mistral-7B-Instruct-v0.2-GGUF".to_string(),
                description: Some("Mistral 7B Instruct v0.2 - GGUF (TheBloke)".to_string()),
                likes: None,
                downloads: None,
            },
        ]
    }

    pub async fn search_models(&self, query: Option<String>) -> AppResult<Vec<HfModelSummary>> {
        let q = query.unwrap_or_default().trim().to_string();
        if q.is_empty() {
            return Ok(self.curated_defaults());
        }
        self.hf.search_models(&q, 25).await
    }

    pub async fn list_gguf_files(&self, repo_id: &str) -> AppResult<Vec<HfFile>> {
        self.hf.list_model_files(repo_id).await
    }

    pub async fn start_download(&self, repo_id: &str, filename: &str, hf_token: Option<&str>) -> AppResult<String> {
        self.model_manager
            .start_download(repo_id.to_string(), filename.to_string(), hf_token.map(|s| s.to_string()))
            .await
    }

    pub async fn list_installed(&self) -> AppResult<Vec<InstalledModel>> {
        self.registry.list_installed().await
    }
}
