//! Error handling module

pub mod types;

pub use types::DistributedDownloaderError;

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, DistributedDownloaderError>;