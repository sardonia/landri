use serde::Serialize;
use tauri::Manager;

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
    fn emit<T: Serialize + ?Sized>(&self, event: &str, payload: &T) -> AppResult<()> {
        self.app
            .emit_all(event, payload)
            .map_err(|e| AppError::Internal(e.to_string()))
    }
}
