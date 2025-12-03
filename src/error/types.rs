//! 分布式下载器的错误类型

use std::fmt;

/// 分布式下载器的自定义错误类型
#[derive(Debug)]
pub enum DistributedDownloaderError {
    /// IO错误
    IoError(std::io::Error),

    /// HTTP错误
    HttpError(String),

    /// 网络错误
    NetworkError(String),

    /// 协议错误
    ProtocolError(String),

    /// 下载错误
    DownloadError(String),
}

impl fmt::Display for DistributedDownloaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DistributedDownloaderError::IoError(e) => write!(f, "IO错误: {}", e),
            DistributedDownloaderError::HttpError(e) => write!(f, "HTTP错误: {}", e),
            DistributedDownloaderError::NetworkError(e) => write!(f, "网络错误: {}", e),
            DistributedDownloaderError::ProtocolError(e) => write!(f, "协议错误: {}", e),
            DistributedDownloaderError::DownloadError(e) => write!(f, "下载错误: {}", e),
        }
    }
}

impl std::error::Error for DistributedDownloaderError {}

impl From<std::io::Error> for DistributedDownloaderError {
    fn from(error: std::io::Error) -> Self {
        DistributedDownloaderError::IoError(error)
    }
}



impl From<reqwest::Error> for DistributedDownloaderError {
    fn from(error: reqwest::Error) -> Self {
        DistributedDownloaderError::NetworkError(format!("HTTP请求错误: {}", error))
    }
}



impl From<tokio::task::JoinError> for DistributedDownloaderError {
    fn from(error: tokio::task::JoinError) -> Self {
        DistributedDownloaderError::NetworkError(format!("任务连接错误: {}", error))
    }
}

impl From<prost::DecodeError> for DistributedDownloaderError {
    fn from(error: prost::DecodeError) -> Self {
        DistributedDownloaderError::ProtocolError(format!("Protobuf解码错误: {}", error))
    }
}