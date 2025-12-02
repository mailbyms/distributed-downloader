//! Distributed Downloader - A high-performance distributed file downloader
//!
//! This crate provides the core functionality for a distributed file downloading system
//! with manager, server, and client components.

/// Configuration module for all components
pub mod config;

/// Network communication module
pub mod network;

/// Download functionality
pub mod downloader;

/// Utility functions and helpers
pub mod utils;

/// Error types and handling
pub mod error;

/// Protobuf generated messages
pub mod proto;

// Re-export commonly used types
pub use config::{ClientConfig, ServerConfig, ManagerConfig};
pub use downloader::HttpDownloader;
pub use error::DistributedDownloaderError;