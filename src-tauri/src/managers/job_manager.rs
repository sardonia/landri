use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct JobManager {
    inner: Arc<Mutex<HashMap<String, CancellationToken>>>,
}

impl JobManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn create_job(&self) -> (String, CancellationToken) {
        let job_id = Uuid::new_v4().to_string();
        let token = CancellationToken::new();
        self.inner.lock().await.insert(job_id.clone(), token.clone());
        (job_id, token)
    }

    pub async fn cancel(&self, job_id: &str) -> bool {
        let token = { self.inner.lock().await.get(job_id).cloned() };
        if let Some(t) = token {
            t.cancel();
            true
        } else {
            false
        }
    }

    pub async fn remove(&self, job_id: &str) {
        self.inner.lock().await.remove(job_id);
    }

    pub async fn get(&self, job_id: &str) -> Option<CancellationToken> {
        self.inner.lock().await.get(job_id).cloned()
    }
}
