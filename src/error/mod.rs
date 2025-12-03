//! 错误处理模块

pub mod types;

pub use types::DistributedDownloaderError;

/// 结果类型别名，为了方便使用
pub type Result<T> = std::result::Result<T, DistributedDownloaderError>;