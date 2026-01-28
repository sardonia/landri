use serde_json::Value;
use tauri::Emitter;

use crate::{
    error::{AppError, AppResult},
    ports::events::EventsPort,
};

#[derive(Clone)]
pub struct TauriEventEmitter {
    app: tauri::AppHandle,
}

impl TauriEventEmitter {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl EventsPort for TauriEventEmitter {
    fn emit(&self, event: &str, payload: &Value) -> AppResult<()> {
        self.app
            .emit(event, payload)
            .map_err(|e: tauri::Error| AppError::Internal(e.to_string()))
    }
}
