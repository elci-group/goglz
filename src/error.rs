// SPDX-License-Identifier: MIT
use std::fmt;

#[derive(Debug)]
pub enum GoglzError {
    Config(anyhow::Error),
    Io(std::io::Error),
    ApiRequest(reqwest::Error),
    FileTooLarge(u64),
    UnsupportedFormat(String),
    ProcessingFailed(String),
    Serialization(serde_json::Error),
    Daemonization(String),
    Watcher(String),
}

impl fmt::Display for GoglzError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoglzError::Config(e) => write!(f, "Configuration error: {e}"),
            GoglzError::Io(e) => write!(f, "IO error: {e}"),
            GoglzError::ApiRequest(e) => write!(f, "API request failed: {e}"),
            GoglzError::FileTooLarge(size) => write!(f, "File too large: {size} MB exceeds limit"),
            GoglzError::UnsupportedFormat(format) => write!(f, "Unsupported file format: {format}"),
            GoglzError::ProcessingFailed(msg) => write!(f, "Processing failed: {msg}"),
            GoglzError::Serialization(e) => write!(f, "Serialization error: {e}"),
            GoglzError::Daemonization(msg) => write!(f, "Daemonization error: {msg}"),
            GoglzError::Watcher(msg) => write!(f, "Watcher error: {msg}"),
        }
    }
}

impl std::error::Error for GoglzError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GoglzError::Config(e) => Some(e.as_ref()),
            GoglzError::Io(e) => Some(e),
            GoglzError::ApiRequest(e) => Some(e),
            GoglzError::Serialization(e) => Some(e),
            _ => None,
        }
    }
}

impl From<anyhow::Error> for GoglzError {
    fn from(e: anyhow::Error) -> Self {
        GoglzError::Config(e)
    }
}

impl From<std::io::Error> for GoglzError {
    fn from(e: std::io::Error) -> Self {
        GoglzError::Io(e)
    }
}

impl From<reqwest::Error> for GoglzError {
    fn from(e: reqwest::Error) -> Self {
        GoglzError::ApiRequest(e)
    }
}

impl From<serde_json::Error> for GoglzError {
    fn from(e: serde_json::Error) -> Self {
        GoglzError::Serialization(e)
    }
}

impl From<notify::Error> for GoglzError {
    fn from(e: notify::Error) -> Self {
        GoglzError::Watcher(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, GoglzError>;
