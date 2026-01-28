use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};

/// Object-safe event emitter port (backend -> frontend).
///
/// NOTE: object safety matters because we store this behind `Arc<dyn EventsPort>`.
pub trait EventsPort: Send + Sync {
    fn emit(&self, event: &str, payload: &Value) -> AppResult<()>;
}

/// Convenience helper to emit any serializable payload through an object-safe `EventsPort`.
pub fn emit_ser<T: Serialize>(events: &dyn EventsPort, event: &str, payload: &T) -> AppResult<()> {
    let v = serde_json::to_value(payload).map_err(|e| AppError::Internal(e.to_string()))?;
    events.emit(event, &v)
}
