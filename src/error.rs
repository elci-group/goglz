use thiserror::Error;

#[derive(Error, Debug)]
pub enum GoglzError {
    #[error("Configuration error: {0}")]
    Config(#[from] anyhow::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("API request failed: {0}")]
    ApiRequest(#[from] reqwest::Error),

    #[error("File too large: {0} MB exceeds limit")]
    FileTooLarge(u64),

    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),

    #[error("Processing failed: {0}")]
    ProcessingFailed(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Daemonization error: {0}")]
    Daemonization(String),

    #[error("Watcher error: {0}")]
    Watcher(String),
}

impl From<notify::Error> for GoglzError {
    fn from(e: notify::Error) -> Self {
        GoglzError::Watcher(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, GoglzError>;
