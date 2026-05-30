use serde::Serialize;
use thiserror::Error;

/// Typed error for all Tauri commands.
/// Serializes as a plain string so the frontend receives a readable message.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("LCU bağlantısı yok")]
    NotConnected,

    #[error("Yapılandırma eksik: {0}")]
    NotConfigured(String),

    #[error("LCU hatası: {0}")]
    Lcu(String),

    #[error("Veritabanı hatası: {0}")]
    Db(String),

    #[error("Ağ hatası: {0}")]
    Network(String),

    #[error("{0}")]
    Other(String),
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Network(e.to_string())
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
