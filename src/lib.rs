//! 分布式下载器 - 一个高性能的分布式文件下载器
//!
//! 这个crate提供了分布式文件下载系统的核心功能
//! 包含管理器、服务器和客户端组件。

/// 所有组件的配置模块
pub mod config;

/// 网络通信模块
pub mod network;

/// 下载功能
pub mod downloader;

/// 实用函数和辅助工具
pub mod utils;

/// 错误类型和处理
pub mod error;

/// Protobuf生成的消息
pub mod proto;

// 重新导出常用的类型
pub use config::{ClientConfig, ServerConfig, ManagerConfig};
pub use downloader::HttpDownloader;
pub use error::DistributedDownloaderError;