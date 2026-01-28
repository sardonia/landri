use serde::Serialize;

use crate::error::AppResult;

pub trait EventsPort: Send + Sync {
    fn emit<T: Serialize + ?Sized>(&self, event: &str, payload: &T) -> AppResult<()>;
}
