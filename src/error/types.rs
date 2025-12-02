//! Error types for the distributed downloader

use std::fmt;

/// Custom error type for the distributed downloader
#[derive(Debug)]
pub enum DistributedDownloaderError {
    /// IO error
    IoError(std::io::Error),

    /// Network error
    NetworkError(String),

    /// Configuration error
    ConfigError(String),

    /// Download error
    DownloadError(String),

    /// Parse error
    ParseError(String),
}

impl fmt::Display for DistributedDownloaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DistributedDownloaderError::IoError(e) => write!(f, "IO error: {}", e),
            DistributedDownloaderError::NetworkError(e) => write!(f, "Network error: {}", e),
            DistributedDownloaderError::ConfigError(e) => write!(f, "Configuration error: {}", e),
            DistributedDownloaderError::DownloadError(e) => write!(f, "Download error: {}", e),
            DistributedDownloaderError::ParseError(e) => write!(f, "Parse error: {}", e),
        }
    }
}

impl std::error::Error for DistributedDownloaderError {}

impl From<std::io::Error> for DistributedDownloaderError {
    fn from(error: std::io::Error) -> Self {
        DistributedDownloaderError::IoError(error)
    }
}

impl From<serde_yaml::Error> for DistributedDownloaderError {
    fn from(error: serde_yaml::Error) -> Self {
        DistributedDownloaderError::ConfigError(format!("YAML parsing error: {}", error))
    }
}

impl From<reqwest::Error> for DistributedDownloaderError {
    fn from(error: reqwest::Error) -> Self {
        DistributedDownloaderError::NetworkError(format!("HTTP request error: {}", error))
    }
}

impl From<serde_json::Error> for DistributedDownloaderError {
    fn from(error: serde_json::Error) -> Self {
        DistributedDownloaderError::ParseError(format!("JSON parsing error: {}", error))
    }
}

impl From<tokio::task::JoinError> for DistributedDownloaderError {
    fn from(error: tokio::task::JoinError) -> Self {
        DistributedDownloaderError::NetworkError(format!("Task join error: {}", error))
    }
}