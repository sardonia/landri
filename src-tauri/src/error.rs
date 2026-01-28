use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Cancelled")]
    Cancelled,

    #[error("Network error: {0}")]
    Network(String),

    #[error("Filesystem error: {0}")]
    Fs(String),

    #[error("Database error: {0}")]
    Db(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Fs(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct ErrorEnvelope {
    pub message: String,
    pub detail: Option<String>,
    pub remediation: Option<String>,
}

impl ErrorEnvelope {
    pub fn simple(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            detail: None,
            remediation: None,
        }
    }

    pub fn with_detail(
        message: impl Into<String>,
        detail: impl Into<String>,
        remediation: Option<String>,
    ) -> Self {
        Self {
            message: message.into(),
            detail: Some(detail.into()),
            remediation,
        }
    }
}
